//! Untrusted build stage.
//!
//! Builds the contestant 5-bit Shor ECDLP circuit and writes the primitive op stream
//! to `ops.bin`. The trusted `eval_circuit` binary re-loads this file in a
//! separate process and does all validation/scoring.

use ecdlp_5_bit::circuit::{Op, OperationType};
use ecdlp_5_bit::shor_oracle;
use std::fs;
use std::path::Path;

const OPS_PATH: &str = "ops.bin";
const MAGIC: &[u8; 8] = b"QECCOPS1";
const OP_BYTES: usize = 56;

fn write_ops(ops: &[Op], path: &Path) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(MAGIC.len() + 8 + ops.len() * OP_BYTES);
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&(ops.len() as u64).to_le_bytes());
    for op in ops {
        buf.extend_from_slice(&(op.kind as u32).to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]);
        buf.extend_from_slice(&op.q_control2.0.to_le_bytes());
        buf.extend_from_slice(&op.q_control1.0.to_le_bytes());
        buf.extend_from_slice(&op.q_target.0.to_le_bytes());
        buf.extend_from_slice(&op.c_target.0.to_le_bytes());
        buf.extend_from_slice(&op.c_condition.0.to_le_bytes());
        buf.extend_from_slice(&op.r_target.0.to_le_bytes());
    }
    let tmp = path.with_extension("bin.tmp");
    fs::write(&tmp, &buf)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn main() {
    println!("=== ecdlp_5_bit: build_circuit (untrusted stage) ===\n");
    println!("-- building circuit --");
    let ops = shor_oracle::build();
    let ccx = ops
        .iter()
        .filter(|op| op.kind == OperationType::CCX)
        .count();
    let ccz = ops
        .iter()
        .filter(|op| op.kind == OperationType::CCZ)
        .count();
    println!("  emitted ops : {}", ops.len());
    println!("  static CCX  : {ccx}");
    println!("  static CCZ  : {ccz}");

    let path = Path::new(OPS_PATH);
    if let Err(error) = write_ops(&ops, path) {
        eprintln!("error: failed to write {OPS_PATH}: {error}");
        std::process::exit(2);
    }
    println!(
        "  wrote       : {} ({} bytes)",
        OPS_PATH,
        ops.len() * OP_BYTES + MAGIC.len() + 8
    );
    println!("\n=== build_circuit OK ===");
}
