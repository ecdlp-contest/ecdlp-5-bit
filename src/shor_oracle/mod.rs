//! Contestant-editable baseline for the 5-bit Shor ECDLP variable-Q oracle.
//!
//! The circuit ABI is:
//!
//! register 0: scalar a, 5 qubits, preserved
//! register 1: scalar b, 5 qubits, preserved
//! register 2: input Q.x, 5 qubits, preserved
//! register 3: input Q.y, 5 qubits, preserved
//! register 4: input Q infinity flag, 1 qubit, preserved
//! register 5: output R.x, 5 qubits, initially zero
//! register 6: output R.y, 5 qubits, initially zero
//! register 7: output R infinity flag, 1 qubit, initially zero
//!
//! The oracle computes:
//!
//! ```text
//! |a>|b>|Q>|0> -> |a>|b>|Q>|aG + bQ>
//! ```
//!
//! for any valid group point `Q` on `y^2 = x^3 + 7 mod 31`. The trusted
//! evaluator chooses `Q = kG` after the circuit is built.
//!
//! This implementation is table-driven, but keeps the intermediate group value
//! as a 5-bit scalar instead of materializing affine point scratch registers:
//!
//! 1. decode the valid input point `Q = kG` into a scratch scalar `k`
//! 2. compute scratch `p = b*k mod 21`
//! 3. table-write output `R = (a+p)G`
//! 4. uncompute `p` and `k`
//!
//! This avoids the baseline's 11-qubit `A`/`B` point scratch and 22-bit point
//! addition table while preserving the public ABI.

use crate::circuit::{Op, OperationType, QubitId, RegisterId};

pub const FIELD_MODULUS: u16 = 31;
pub const CURVE_A: u16 = 0;
pub const GROUP_ORDER: u16 = 21;
pub const WIDTH: usize = 5;

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

struct Builder {
    ops: Vec<Op>,
    next_qubit: u64,
    next_register: u64,
}

impl Builder {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            next_qubit: 0,
            next_register: 0,
        }
    }

    fn push(&mut self, op: Op) {
        op.validate();
        self.ops.push(op);
    }

    fn alloc_qubit(&mut self) -> QubitId {
        let out = QubitId(self.next_qubit);
        self.next_qubit += 1;
        out
    }

    fn alloc_qubits(&mut self, n: usize) -> Vec<QubitId> {
        (0..n).map(|_| self.alloc_qubit()).collect()
    }

    fn declare_qubit_register(&mut self, qubits: &[QubitId]) {
        let register = RegisterId(self.next_register);
        self.next_register += 1;
        for &qubit in qubits {
            let mut op = Op::empty();
            op.kind = OperationType::AppendToRegister;
            op.q_target = qubit;
            op.r_target = register;
            self.push(op);
        }
        let mut op = Op::empty();
        op.kind = OperationType::Register;
        op.r_target = register;
        self.push(op);
    }

    fn x(&mut self, target: QubitId) {
        let mut op = Op::empty();
        op.kind = OperationType::X;
        op.q_target = target;
        self.push(op);
    }

    fn cx(&mut self, control: QubitId, target: QubitId) {
        let mut op = Op::empty();
        op.kind = OperationType::CX;
        op.q_control1 = control;
        op.q_target = target;
        self.push(op);
    }

    fn ccx(&mut self, control1: QubitId, control2: QubitId, target: QubitId) {
        let mut op = Op::empty();
        op.kind = OperationType::CCX;
        op.q_control1 = control1;
        op.q_control2 = control2;
        op.q_target = target;
        self.push(op);
    }
}

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

fn point_add(left: Point, right: Point) -> Point {
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
            acc = point_add(acc, cur);
        }
        cur = point_add(cur, cur);
        scalar >>= 1;
    }
    acc
}

fn group_points() -> [Point; GROUP_ORDER as usize] {
    let mut points = [Point {
        x: 0,
        y: 0,
        infinity: true,
    }; GROUP_ORDER as usize];
    let mut scalar = 1u16;
    while scalar < GROUP_ORDER {
        points[scalar as usize] = scalar_mul(scalar, G);
        scalar += 1;
    }
    points
}

fn point_bits(point: Point) -> u32 {
    let mut out = u32::from(point.x);
    out |= u32::from(point.y) << WIDTH;
    if point.infinity {
        out |= 1 << (2 * WIDTH);
    }
    out
}

fn emit_exact_match_flag(builder: &mut Builder, input: &[QubitId], value: u32, temps: &[QubitId]) {
    debug_assert!(temps.len() + 1 >= input.len());
    for (bit, &qubit) in input.iter().enumerate() {
        if ((value >> bit) & 1) == 0 {
            builder.x(qubit);
        }
    }

    builder.ccx(input[0], input[1], temps[0]);
    for bit in 2..input.len() {
        builder.ccx(temps[bit - 2], input[bit], temps[bit - 1]);
    }
}

fn unemit_exact_match_flag(
    builder: &mut Builder,
    input: &[QubitId],
    value: u32,
    temps: &[QubitId],
) {
    for bit in (2..input.len()).rev() {
        builder.ccx(temps[bit - 2], input[bit], temps[bit - 1]);
    }
    builder.ccx(input[0], input[1], temps[0]);

    for (bit, &qubit) in input.iter().enumerate() {
        if ((value >> bit) & 1) == 0 {
            builder.x(qubit);
        }
    }
}

fn emit_point_xor(
    builder: &mut Builder,
    flag: QubitId,
    point: Point,
    x: &[QubitId],
    y: &[QubitId],
    inf: QubitId,
) {
    if point.infinity {
        builder.cx(flag, inf);
        return;
    }
    for (bit, &target) in x.iter().enumerate() {
        if ((point.x >> bit) & 1) != 0 {
            builder.cx(flag, target);
        }
    }
    for (bit, &target) in y.iter().enumerate() {
        if ((point.y >> bit) & 1) != 0 {
            builder.cx(flag, target);
        }
    }
}

fn emit_scalar_xor(builder: &mut Builder, flag: QubitId, value: u16, target: &[QubitId]) {
    for (bit, &qubit) in target.iter().enumerate() {
        if ((value >> bit) & 1) != 0 {
            builder.cx(flag, qubit);
        }
    }
}

fn emit_q_log_table(
    builder: &mut Builder,
    qx: &[QubitId],
    qy: &[QubitId],
    qinf: QubitId,
    k_out: &[QubitId],
    temps: &[QubitId],
) {
    let points = group_points();
    let mut input = Vec::with_capacity(2 * WIDTH + 1);
    input.extend_from_slice(qx);
    input.extend_from_slice(qy);
    input.push(qinf);

    for (k, &q) in points.iter().enumerate() {
        let value = point_bits(q);
        emit_exact_match_flag(builder, &input, value, temps);
        emit_scalar_xor(builder, temps[input.len() - 2], k as u16, k_out);
        unemit_exact_match_flag(builder, &input, value, temps);
    }
}

fn emit_product_scalar_table(
    builder: &mut Builder,
    b: &[QubitId],
    k: &[QubitId],
    product_out: &[QubitId],
    temps: &[QubitId],
) {
    let mut input = Vec::with_capacity(2 * WIDTH);
    input.extend_from_slice(b);
    input.extend_from_slice(k);

    for raw_b in 0..(1u16 << WIDTH) {
        for raw_k in 0..GROUP_ORDER {
            let value = u32::from(raw_b) | (u32::from(raw_k) << WIDTH);
            let product = ((raw_b % GROUP_ORDER) * raw_k) % GROUP_ORDER;
            emit_exact_match_flag(builder, &input, value, temps);
            emit_scalar_xor(builder, temps[input.len() - 2], product, product_out);
            unemit_exact_match_flag(builder, &input, value, temps);
        }
    }
}

fn emit_result_point_table(
    builder: &mut Builder,
    a: &[QubitId],
    product: &[QubitId],
    rx: &[QubitId],
    ry: &[QubitId],
    rinf: QubitId,
    temps: &[QubitId],
) {
    let mut input = Vec::with_capacity(2 * WIDTH);
    input.extend_from_slice(a);
    input.extend_from_slice(product);

    for raw_a in 0..(1u16 << WIDTH) {
        for raw_product in 0..GROUP_ORDER {
            let value = u32::from(raw_a) | (u32::from(raw_product) << WIDTH);
            let scalar = ((raw_a % GROUP_ORDER) + raw_product) % GROUP_ORDER;
            let point = scalar_mul(scalar, G);
            emit_exact_match_flag(builder, &input, value, temps);
            emit_point_xor(builder, temps[input.len() - 2], point, rx, ry, rinf);
            unemit_exact_match_flag(builder, &input, value, temps);
        }
    }
}

pub fn build() -> Vec<Op> {
    let mut builder = Builder::new();
    let a = builder.alloc_qubits(WIDTH);
    let b = builder.alloc_qubits(WIDTH);
    let qx = builder.alloc_qubits(WIDTH);
    let qy = builder.alloc_qubits(WIDTH);
    let qinf = builder.alloc_qubit();
    let rx = builder.alloc_qubits(WIDTH);
    let ry = builder.alloc_qubits(WIDTH);
    let rinf = builder.alloc_qubit();

    let q_log = builder.alloc_qubits(WIDTH);
    let product = builder.alloc_qubits(WIDTH);
    let temps = builder.alloc_qubits(2 * WIDTH);

    builder.declare_qubit_register(&a);
    builder.declare_qubit_register(&b);
    builder.declare_qubit_register(&qx);
    builder.declare_qubit_register(&qy);
    builder.declare_qubit_register(&[qinf]);
    builder.declare_qubit_register(&rx);
    builder.declare_qubit_register(&ry);
    builder.declare_qubit_register(&[rinf]);

    emit_q_log_table(&mut builder, &qx, &qy, qinf, &q_log, &temps);
    emit_product_scalar_table(&mut builder, &b, &q_log, &product, &temps);
    emit_result_point_table(&mut builder, &a, &product, &rx, &ry, rinf, &temps);
    emit_product_scalar_table(&mut builder, &b, &q_log, &product, &temps);
    emit_q_log_table(&mut builder, &qx, &qy, qinf, &q_log, &temps);

    builder.ops
}
