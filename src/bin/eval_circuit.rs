//! Trusted evaluation stage.
//!
//! Reads the op stream emitted by `build_circuit` from `ops.bin`, validates the
//! framing, simulates 9024 Fiat-Shamir 5-bit Shor ECDLP variable-Q oracle
//! shots, checks phase and ancilla cleanup, then writes `score.json` and
//! `results.tsv`.

use alloy_primitives::U256;
use ecdlp_5_bit::circuit::{
    analyze_ops, BitId, Op, OperationType, QubitId, QubitOrBit, RegisterId,
};
use ecdlp_5_bit::sim::Simulator;
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const OPS_PATH: &str = "ops.bin";
const MAGIC: &[u8; 8] = b"QECCOPS1";
const OP_BYTES: usize = 56;
const MAX_OPS: u64 = 4_000_000_000;
const NUM_TESTS: usize = 9024;
const RESULTS_HEADER: &str =
    "timestamp\tcommit\ttoffoli\tccx\tccz\tclifford\tqubits\tops\tstatus\tnote\n";
const SCORE_MODEL: &str = "primitive-ccx-ccz-v1";
const VALIDATION_GATE: &str = "fiat_shamir_shor_ecdlp_5bit_variable_q_oracle";
const FIELD_MODULUS: u16 = 31;
const CURVE_A: u16 = 0;
const GROUP_ORDER: u16 = 21;
const WIDTH: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    x: u16,
    y: u16,
    infinity: bool,
}

const G: Point = Point {
    x: 1,
    y: 15,
    infinity: false,
};

fn mod_i(value: i32) -> u16 {
    value.rem_euclid(FIELD_MODULUS as i32) as u16
}

fn inv_mod(value: u16) -> Option<u16> {
    if value == 0 {
        return None;
    }
    let mut t = 0i32;
    let mut new_t = 1i32;
    let mut r = FIELD_MODULUS as i32;
    let mut new_r = value as i32;
    while new_r != 0 {
        let quotient = r / new_r;
        (t, new_t) = (new_t, t - quotient * new_t);
        (r, new_r) = (new_r, r - quotient * new_r);
    }
    if r > 1 {
        None
    } else {
        Some(mod_i(t))
    }
}

fn curve_add(left: Point, right: Point) -> Point {
    if left.infinity {
        return right;
    }
    if right.infinity {
        return left;
    }
    if left.x == right.x && (left.y + right.y) % FIELD_MODULUS == 0 {
        return Point {
            x: 0,
            y: 0,
            infinity: true,
        };
    }

    let lambda = if left == right {
        if left.y == 0 {
            return Point {
                x: 0,
                y: 0,
                infinity: true,
            };
        }
        let numerator = (3 * left.x as i32 * left.x as i32 + CURVE_A as i32) % FIELD_MODULUS as i32;
        let denominator = inv_mod((2 * left.y) % FIELD_MODULUS).expect("nonzero tangent");
        mod_i(numerator * denominator as i32)
    } else {
        let numerator = mod_i(right.y as i32 - left.y as i32);
        let denominator =
            inv_mod(mod_i(right.x as i32 - left.x as i32)).expect("nonzero addition denominator");
        mod_i(numerator as i32 * denominator as i32)
    };
    let x3 = mod_i(lambda as i32 * lambda as i32 - left.x as i32 - right.x as i32);
    let y3 = mod_i(lambda as i32 * (left.x as i32 - x3 as i32) - left.y as i32);
    Point {
        x: x3,
        y: y3,
        infinity: false,
    }
}

fn scalar_mul(mut scalar: u16, point: Point) -> Point {
    scalar %= GROUP_ORDER;
    let mut acc = Point {
        x: 0,
        y: 0,
        infinity: true,
    };
    let mut cur = point;
    while scalar != 0 {
        if scalar & 1 != 0 {
            acc = curve_add(acc, cur);
        }
        cur = curve_add(cur, cur);
        scalar >>= 1;
    }
    acc
}

fn oracle_point(a: u16, b: u16, q: Point) -> Point {
    curve_add(
        scalar_mul(a % GROUP_ORDER, G),
        scalar_mul(b % GROUP_ORDER, q),
    )
}

fn assert_public_instance() {
    assert!(scalar_mul(GROUP_ORDER, G).infinity, "G must have order 31");
}

fn op_kind_from_u32(v: u32) -> Option<OperationType> {
    Some(match v {
        0 => OperationType::Neg,
        1 => OperationType::Register,
        2 => OperationType::AppendToRegister,
        3 => OperationType::BitInvert,
        4 => OperationType::BitStore0,
        5 => OperationType::BitStore1,
        6 => OperationType::X,
        7 => OperationType::Z,
        8 => OperationType::CX,
        9 => OperationType::CZ,
        10 => OperationType::Swap,
        11 => OperationType::R,
        12 => OperationType::Hmr,
        13 => OperationType::CCX,
        14 => OperationType::CCZ,
        15 => OperationType::PushCondition,
        16 => OperationType::PopCondition,
        17 => OperationType::DebugPrint,
        _ => return None,
    })
}

fn read_u64(bytes: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
}

fn load_ops(path: &str) -> Result<Vec<Op>, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {path}: {error}"))?;
    if bytes.len() < MAGIC.len() + 8 {
        return Err(format!("{path}: too short ({} bytes)", bytes.len()));
    }
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err(format!("{path}: bad magic"));
    }
    let n = u64::from_le_bytes(bytes[MAGIC.len()..MAGIC.len() + 8].try_into().unwrap());
    if n > MAX_OPS {
        return Err(format!("{path}: op count {n} exceeds cap {MAX_OPS}"));
    }
    let n = n as usize;
    let need = MAGIC.len() + 8 + n.saturating_mul(OP_BYTES);
    if bytes.len() != need {
        return Err(format!(
            "{path}: length mismatch: got {} expected {need} for {n} ops",
            bytes.len()
        ));
    }

    let mut ops = Vec::with_capacity(n);
    let mut off = MAGIC.len() + 8;
    for i in 0..n {
        let kind_raw = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let kind =
            op_kind_from_u32(kind_raw).ok_or_else(|| format!("op {i}: unknown kind {kind_raw}"))?;
        let op = Op {
            kind,
            q_control2: QubitId(read_u64(&bytes, off + 8)),
            q_control1: QubitId(read_u64(&bytes, off + 16)),
            q_target: QubitId(read_u64(&bytes, off + 24)),
            c_target: BitId(read_u64(&bytes, off + 32)),
            c_condition: BitId(read_u64(&bytes, off + 40)),
            r_target: RegisterId(read_u64(&bytes, off + 48)),
        };
        let validated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| op.validate()));
        if let Err(error) = validated {
            let message = error
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| error.downcast_ref::<&'static str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "validation panic".to_string());
            return Err(format!("op {i}: {message}"));
        }
        ops.push(op);
        off += OP_BYTES;
    }
    Ok(ops)
}

fn fiat_shamir_seed(ops: &[Op]) -> sha3::Shake256Reader {
    let mut hasher = Shake256::default();
    hasher.update(b"ecdlp-shor-ecdlp-5bit-variable-q-oracle-fiat-shamir-v1");
    hasher.update(&(ops.len() as u64).to_le_bytes());
    for op in ops {
        hasher.update(&[op.kind as u8]);
        hasher.update(&op.q_control2.0.to_le_bytes());
        hasher.update(&op.q_control1.0.to_le_bytes());
        hasher.update(&op.q_target.0.to_le_bytes());
        hasher.update(&op.c_target.0.to_le_bytes());
        hasher.update(&op.c_condition.0.to_le_bytes());
        hasher.update(&op.r_target.0.to_le_bytes());
    }
    hasher.finalize_xof()
}

struct SeedReport {
    ok: bool,
    avg_cliff: f64,
    avg_tof: f64,
    avg_ccx: f64,
    avg_ccz: f64,
    tot_tof: u64,
    tot_ccx: u64,
    tot_ccz: u64,
    tot_cliff: u64,
    n_shots: usize,
    oracle_failures: usize,
    input_failures: usize,
    phase_garbage_batches: usize,
    ancilla_garbage_batches: usize,
    fail_reason: Option<String>,
}

fn run_tests(
    ops: &[Op],
    layout_regs: &[Vec<QubitOrBit>],
    total_qubits: u64,
    num_bits: u64,
    mut xof: sha3::Shake256Reader,
    target_shots: usize,
) -> SeedReport {
    let mut inputs = Vec::with_capacity(target_shots);
    while inputs.len() < target_shots {
        let mut bytes = [0u8; 3];
        xof.read(&mut bytes);
        let a = u16::from(bytes[0]) & ((1u16 << WIDTH) - 1);
        let b = u16::from(bytes[1]) & ((1u16 << WIDTH) - 1);
        let q_scalar = u16::from(bytes[2]) % GROUP_ORDER;
        inputs.push((a, b, scalar_mul(q_scalar, G)));
    }

    let n = inputs.len();
    let mut sim = Simulator::new(total_qubits as usize, num_bits as usize, &mut xof);
    let mut ok = true;
    let mut fail_reason: Option<String> = None;
    let mut oracle_failures = 0usize;
    let mut input_failures = 0usize;
    let mut phase_garbage_batches = 0usize;
    let mut ancilla_garbage_batches = 0usize;

    const BATCH: usize = 64;
    let num_batches = n.div_ceil(BATCH);
    for batch in 0..num_batches {
        let bs = BATCH.min(n - batch * BATCH);
        let cond_mask: u64 = if bs == 64 { u64::MAX } else { (1u64 << bs) - 1 };

        sim.clear_for_shot();
        for shot in 0..bs {
            let i = batch * BATCH + shot;
            let (a, b, q) = inputs[i];
            sim.set_register(&layout_regs[0], U256::from(a), shot);
            sim.set_register(&layout_regs[1], U256::from(b), shot);
            sim.set_register(&layout_regs[2], U256::from(q.x), shot);
            sim.set_register(&layout_regs[3], U256::from(q.y), shot);
            sim.set_register(&layout_regs[4], U256::from(u16::from(q.infinity)), shot);
        }

        sim.apply_iter(ops.iter());

        for shot in 0..bs {
            let i = batch * BATCH + shot;
            let (a_expected, b_expected, q_expected) = inputs[i];
            let a_in = sim.get_register(&layout_regs[0], shot).to::<u16>();
            let b_in = sim.get_register(&layout_regs[1], shot).to::<u16>();
            let qx_in = sim.get_register(&layout_regs[2], shot).to::<u16>();
            let qy_in = sim.get_register(&layout_regs[3], shot).to::<u16>();
            let qinf_in = sim.get_register(&layout_regs[4], shot).to::<u16>() != 0;
            if a_in != a_expected
                || b_in != b_expected
                || qx_in != q_expected.x
                || qy_in != q_expected.y
                || qinf_in != q_expected.infinity
            {
                input_failures += 1;
                if fail_reason.is_none() {
                    fail_reason = Some(format!(
                        "INPUT MISMATCH shot {i}: expected a={a_expected} b={b_expected} Q=({},{},inf={}), got a={a_in} b={b_in} Q=({qx_in},{qy_in},inf={qinf_in})",
                        q_expected.x, q_expected.y, q_expected.infinity
                    ));
                }
                ok = false;
                continue;
            }

            let got_x = sim.get_register(&layout_regs[5], shot).to::<u16>();
            let got_y = sim.get_register(&layout_regs[6], shot).to::<u16>();
            let got_inf = sim.get_register(&layout_regs[7], shot).to::<u16>() != 0;
            let expected = oracle_point(a_expected, b_expected, q_expected);
            let output_ok = if expected.infinity {
                got_inf && got_x == 0 && got_y == 0
            } else {
                !got_inf && got_x == expected.x && got_y == expected.y
            };
            if !output_ok {
                oracle_failures += 1;
                if fail_reason.is_none() {
                    fail_reason = Some(format!(
                        "ORACLE MISMATCH shot {i}: a={a_expected} b={b_expected} Q=({},{},inf={}) got=({got_x},{got_y},inf={got_inf}) expected=({},{},inf={})",
                        q_expected.x,
                        q_expected.y,
                        q_expected.infinity,
                        expected.x,
                        expected.y,
                        expected.infinity
                    ));
                }
                ok = false;
            }
        }

        let phase = sim.phase & cond_mask;
        if phase != 0 {
            phase_garbage_batches += 1;
            if fail_reason.is_none() {
                fail_reason = Some(format!(
                    "PHASE GARBAGE: global_phase = {phase:#018x} across {bs} live shots"
                ));
            }
            ok = false;
        }

        for register in layout_regs {
            for wire in register {
                if let QubitOrBit::Qubit(q) = *wire {
                    *sim.qubit_mut(q) = 0;
                }
            }
        }
        let mut garbage_q: Option<u64> = None;
        for q in 0..total_qubits {
            let value = sim.qubit(QubitId(q)) & cond_mask;
            if value != 0 {
                garbage_q = Some(q);
                break;
            }
        }
        if let Some(q) = garbage_q {
            ancilla_garbage_batches += 1;
            let value = sim.qubit(QubitId(q)) & cond_mask;
            if fail_reason.is_none() {
                fail_reason = Some(format!(
                    "ANCILLA GARBAGE: qubit {q} = {value:#018x}; every non-register qubit must end in zero"
                ));
            }
            ok = false;
        }
    }

    let denom = n.max(1) as f64;
    SeedReport {
        ok,
        avg_cliff: sim.stats.clifford_gates as f64 / denom,
        avg_tof: sim.stats.toffoli_gates as f64 / denom,
        avg_ccx: sim.stats.ccx_gates as f64 / denom,
        avg_ccz: sim.stats.ccz_gates as f64 / denom,
        tot_tof: sim.stats.toffoli_gates,
        tot_ccx: sim.stats.ccx_gates,
        tot_ccz: sim.stats.ccz_gates,
        tot_cliff: sim.stats.clifford_gates,
        n_shots: n,
        oracle_failures,
        input_failures,
        phase_garbage_batches,
        ancilla_garbage_batches,
        fail_reason,
    }
}

fn parse_note() -> String {
    let mut args = std::env::args().skip(1);
    let mut note = String::new();
    while let Some(arg) = args.next() {
        if arg == "--note" {
            if let Some(value) = args.next() {
                note = value;
            }
        } else if let Some(rest) = arg.strip_prefix("--note=") {
            note = rest.to_string();
        }
    }
    note.replace('\t', " ").replace('\n', " ")
}

fn git_commit_short() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "nogit".to_string())
}

fn append_results_row(
    status: &str,
    avg_tof: f64,
    avg_ccx: f64,
    avg_ccz: f64,
    avg_cliff: f64,
    qubits: u64,
    ops_len: usize,
    note: &str,
) {
    let path = Path::new("results.tsv");
    if !path.exists()
        || fs::read_to_string(path)
            .ok()
            .and_then(|body| {
                body.lines()
                    .next()
                    .map(|line| line != RESULTS_HEADER.trim_end())
            })
            .unwrap_or(true)
    {
        fs::write(path, RESULTS_HEADER).expect("write results.tsv header");
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let row = format!(
        "{ts}\t{}\t{avg_tof:.3}\t{avg_ccx:.3}\t{avg_ccz:.3}\t{avg_cliff:.3}\t{qubits}\t{ops_len}\t{status}\t{}\n",
        git_commit_short(),
        note.replace('\t', " ").replace('\n', " ")
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open results.tsv");
    file.write_all(row.as_bytes()).expect("append results row");
}

fn write_score(
    avg_tof: f64,
    avg_ccx: f64,
    avg_ccz: f64,
    avg_cliff: f64,
    qubits: u64,
    ops_len: usize,
    shots: usize,
) {
    let toffoli = avg_tof.round() as u64;
    let ccx = avg_ccx.round() as u64;
    let ccz = avg_ccz.round() as u64;
    let clifford = avg_cliff.round() as u64;
    let score = toffoli.saturating_mul(qubits);
    let body = format!(
        "{{\n  \"score\": {score},\n  \"score_model\": \"{SCORE_MODEL}\",\n  \"metrics\": {{\n    \"toffoli\": {toffoli},\n    \"ccx\": {ccx},\n    \"ccz\": {ccz},\n    \"clifford\": {clifford},\n    \"qubits\": {qubits},\n    \"ops\": {ops_len}\n  }},\n  \"validation\": {{\n    \"shots\": {shots},\n    \"gate\": \"{VALIDATION_GATE}\",\n    \"checks\": [\"oracle correctness\", \"input preservation\", \"phase cleanliness\", \"ancilla cleanup\"]\n  }},\n  \"artifact\": \"{OPS_PATH}\",\n  \"status\": \"ranked\"\n}}\n"
    );
    fs::write("score.json", body).expect("write score.json");
}

fn fail_and_exit(reason: &str, note: &str, ops_len: usize, total_qubits: u64) -> ! {
    eprintln!("\n!! eval_circuit FAILED: {reason}");
    let fail_note = if note.is_empty() {
        reason.to_string()
    } else {
        format!("{note} | {reason}")
    };
    append_results_row(
        "FAIL",
        0.0,
        0.0,
        0.0,
        0.0,
        total_qubits,
        ops_len,
        &fail_note,
    );
    std::process::exit(1);
}

fn main() {
    assert_public_instance();
    let note = parse_note();
    println!("=== ecdlp_5_bit: eval_circuit (trusted stage) ===\n");
    println!("  curve      : y^2 = x^3 + 7 mod 31");
    println!("  generator  : G=({}, {})", G.x, G.y);
    println!("  Q input    : any valid point Q=kG");
    println!("  oracle     : |a>|b>|Q>|0> -> |a>|b>|Q>|aG + bQ>");

    let ops = match load_ops(OPS_PATH) {
        Ok(ops) => ops,
        Err(error) => {
            eprintln!("!! could not load {OPS_PATH}: {error}");
            append_results_row(
                "FAIL",
                0.0,
                0.0,
                0.0,
                0.0,
                0,
                0,
                &format!("{note} | load: {error}"),
            );
            std::process::exit(1);
        }
    };
    println!("  loaded ops : {}", ops.len());

    let (total_qubits, num_bits, _num_regs, regs) = analyze_ops(ops.iter());
    if regs.len() != 8 {
        fail_and_exit(
            &format!("expected 8 registers, got {}", regs.len()),
            &note,
            ops.len(),
            total_qubits,
        );
    }
    for (i, register) in regs.iter().enumerate() {
        let expected_width = if i == 4 || i == 7 { 1 } else { WIDTH };
        if register.len() != expected_width {
            fail_and_exit(
                &format!(
                    "register {i} should be {expected_width} wide, got {}",
                    register.len()
                ),
                &note,
                ops.len(),
                total_qubits,
            );
        }
        if !register
            .iter()
            .all(|wire| matches!(wire, QubitOrBit::Qubit(_)))
        {
            fail_and_exit(
                &format!("register {i} must contain qubits"),
                &note,
                ops.len(),
                total_qubits,
            );
        }
    }

    println!("  qubits     : {}", total_qubits);
    println!("  bits       : {}", num_bits);

    println!("\n-- Fiat-Shamir oracle tests ({NUM_TESTS} shots) --");
    let xof = fiat_shamir_seed(&ops);
    let report = run_tests(&ops, &regs, total_qubits, num_bits, xof, NUM_TESTS);
    println!("  tested shots            : {}", report.n_shots);
    println!("  input failures          : {}", report.input_failures);
    println!("  oracle failures         : {}", report.oracle_failures);
    println!(
        "  phase-garbage batches   : {}",
        report.phase_garbage_batches
    );
    println!(
        "  ancilla-garbage batches : {}",
        report.ancilla_garbage_batches
    );
    if !report.ok {
        let reason = report
            .fail_reason
            .clone()
            .unwrap_or_else(|| "(no detail)".to_string());
        let fail_note = if note.is_empty() {
            reason.clone()
        } else {
            format!("{note} | {reason}")
        };
        append_results_row(
            "FAIL",
            report.avg_tof,
            report.avg_ccx,
            report.avg_ccz,
            report.avg_cliff,
            total_qubits,
            ops.len(),
            &fail_note,
        );
        eprintln!("\n!! correctness FAILED: {reason}");
        std::process::exit(1);
    }
    println!("  all {} shots OK", report.n_shots);

    println!("\n=== circuit metrics (5-bit Shor ECDLP oracle) ===");
    println!("  avg executed Toffoli  : {:.3}", report.avg_tof);
    println!("  avg executed CCX      : {:.3}", report.avg_ccx);
    println!("  avg executed CCZ      : {:.3}", report.avg_ccz);
    println!("  avg executed Clifford : {:.3}", report.avg_cliff);
    println!(
        "  total Toffoli (sum)   : {} over {} shots",
        report.tot_tof, report.n_shots
    );
    println!("  total CCX (sum)       : {}", report.tot_ccx);
    println!("  total CCZ (sum)       : {}", report.tot_ccz);
    println!("  total Clifford (sum)  : {}", report.tot_cliff);
    println!("  emitted ops           : {}", ops.len());
    println!("  qubits                : {}", total_qubits);

    append_results_row(
        "OK",
        report.avg_tof,
        report.avg_ccx,
        report.avg_ccz,
        report.avg_cliff,
        total_qubits,
        ops.len(),
        &note,
    );
    write_score(
        report.avg_tof,
        report.avg_ccx,
        report.avg_ccz,
        report.avg_cliff,
        total_qubits,
        ops.len(),
        report.n_shots,
    );

    println!("\n=== eval_circuit OK ===");
}
