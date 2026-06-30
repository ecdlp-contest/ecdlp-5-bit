//! Contestant-editable baseline for the 5-bit Shor ECDLP variable-base oracle.
//!
//! The circuit ABI is:
//!
//! register 0: scalar a, 5 qubits, preserved
//! register 1: scalar b, 5 qubits, preserved
//! register 2: input P.x, 5 qubits, preserved
//! register 3: input P.y, 5 qubits, preserved
//! register 4: input P infinity flag, 1 qubit, preserved
//! register 5: input Q.x, 5 qubits, preserved
//! register 6: input Q.y, 5 qubits, preserved
//! register 7: input Q infinity flag, 1 qubit, preserved
//! register 8: output R.x, 5 qubits, initially zero
//! register 9: output R.y, 5 qubits, initially zero
//! register 10: output R infinity flag, 1 qubit, initially zero
//! register 11: output (P+Q).x, 5 qubits, initially zero
//! register 12: output (P+Q).y, 5 qubits, initially zero
//! register 13: output (P+Q) infinity flag, 1 qubit, initially zero
//! register 14: output (2P).x, 5 qubits, initially zero
//! register 15: output (2P).y, 5 qubits, initially zero
//! register 16: output (2P) infinity flag, 1 qubit, initially zero
//! register 17: field selector, 2 qubits, preserved (0=F_31, 1=F_13, 2=F_11)
//! register 18: field input x1, 5 qubits, preserved
//! register 19: field input y1, 5 qubits, preserved
//! register 20: field input x2, 5 qubits, preserved
//! register 21: field input y2, 5 qubits, preserved
//! register 22: field output x1+x2, 5 qubits, initially zero
//! register 23: field output denominator x2-x1, 5 qubits, initially zero
//! register 24: field output numerator y2-y1, 5 qubits, initially zero
//! register 25: field output x1*y2, 5 qubits, initially zero
//! register 26: field output (x2-x1)^-1 or 0, 5 qubits, initially zero
//! register 27: field output lambda=(y2-y1)/(x2-x1) or 0, 5 qubits, initially zero
//! register 28: field output lambda*(x2-x1), 5 qubits, initially zero
//!
//! The oracle computes:
//!
//! ```text
//! |a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>|P+Q>|2P>
//! ```
//!
//! for valid group points `P` and `Q` on `y^2 = x^3 + 7 mod 31`. The trusted
//! evaluator chooses both points after the circuit is built.
//!
//! This baseline is table-driven:
//!
//! 1. compute scratch `A = aP`
//! 2. compute scratch `B = bQ`
//! 3. compute output `R = A + B`
//! 4. compute the explicit point-operation checks `P+Q` and `2P`
//! 5. uncompute `B` and `A`
//!
//! It is intentionally simple and CCX-heavy so contenders can replace the
//! tables with arithmetic.

use crate::circuit::{Op, OperationType, QubitId, RegisterId};

pub const FIELD_MODULUS: u16 = 31;
pub const FIELD_SELECTOR_WIDTH: usize = 2;
pub const FIELD_TEST_WIDTH: usize = 5;
pub const FIELD_SPECS: [(u16, u16); 3] = [(0, 31), (1, 13), (2, 11)];
pub const CURVE_A: u16 = 0;
pub const GROUP_ORDER: u16 = 21;
pub const WIDTH: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    x: u16,
    y: u16,
    infinity: bool,
}

const BASE_POINT: Point = Point {
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
        points[scalar as usize] = scalar_mul(scalar, BASE_POINT);
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

fn emit_scalar_point_table(
    builder: &mut Builder,
    scalar: &[QubitId],
    point_x: &[QubitId],
    point_y: &[QubitId],
    point_inf: QubitId,
    out_x: &[QubitId],
    out_y: &[QubitId],
    out_inf: QubitId,
    temps: &[QubitId],
) {
    let points = group_points();
    let mut input = Vec::with_capacity(WIDTH + 2 * WIDTH + 1);
    input.extend_from_slice(scalar);
    input.extend_from_slice(point_x);
    input.extend_from_slice(point_y);
    input.push(point_inf);

    for raw_scalar in 0..(1u16 << WIDTH) {
        for &input_point in &points {
            let value = u32::from(raw_scalar) | (point_bits(input_point) << WIDTH);
            let point = scalar_mul(raw_scalar % GROUP_ORDER, input_point);
            emit_exact_match_flag(builder, &input, value, temps);
            emit_point_xor(
                builder,
                temps[input.len() - 2],
                point,
                out_x,
                out_y,
                out_inf,
            );
            unemit_exact_match_flag(builder, &input, value, temps);
        }
    }
}

fn emit_point_add_table(
    builder: &mut Builder,
    ax: &[QubitId],
    ay: &[QubitId],
    ainf: QubitId,
    bx: &[QubitId],
    by: &[QubitId],
    binf: QubitId,
    rx: &[QubitId],
    ry: &[QubitId],
    rinf: QubitId,
    temps: &[QubitId],
) {
    let points = group_points();
    let mut input = Vec::with_capacity(2 * (2 * WIDTH + 1));
    input.extend_from_slice(ax);
    input.extend_from_slice(ay);
    input.push(ainf);
    input.extend_from_slice(bx);
    input.extend_from_slice(by);
    input.push(binf);

    for &left in &points {
        for &right in &points {
            let value = point_bits(left) | (point_bits(right) << (2 * WIDTH + 1));
            let point = point_add(left, right);
            emit_exact_match_flag(builder, &input, value, temps);
            emit_point_xor(builder, temps[input.len() - 2], point, rx, ry, rinf);
            unemit_exact_match_flag(builder, &input, value, temps);
        }
    }
}

fn emit_point_double_table(
    builder: &mut Builder,
    point_x: &[QubitId],
    point_y: &[QubitId],
    point_inf: QubitId,
    out_x: &[QubitId],
    out_y: &[QubitId],
    out_inf: QubitId,
    temps: &[QubitId],
) {
    let points = group_points();
    let mut input = Vec::with_capacity(2 * WIDTH + 1);
    input.extend_from_slice(point_x);
    input.extend_from_slice(point_y);
    input.push(point_inf);

    for &point in &points {
        let value = point_bits(point);
        let doubled = point_add(point, point);
        emit_exact_match_flag(builder, &input, value, temps);
        emit_point_xor(
            builder,
            temps[input.len() - 2],
            doubled,
            out_x,
            out_y,
            out_inf,
        );
        unemit_exact_match_flag(builder, &input, value, temps);
    }
}

fn test_add(left: u16, right: u16, modulus: u16) -> u16 {
    (left + right) % modulus
}

fn test_sub(left: u16, right: u16, modulus: u16) -> u16 {
    (left as i32 - right as i32).rem_euclid(modulus as i32) as u16
}

fn test_mul(left: u16, right: u16, modulus: u16) -> u16 {
    ((left as u32 * right as u32) % modulus as u32) as u16
}

fn test_inv(value: u16, modulus: u16) -> u16 {
    if value == 0 {
        return 0;
    }
    let mut t = 0i32;
    let mut new_t = 1i32;
    let mut r = modulus as i32;
    let mut new_r = value as i32;
    while new_r != 0 {
        let quotient = r / new_r;
        (t, new_t) = (new_t, t - quotient * new_t);
        (r, new_r) = (new_r, r - quotient * new_r);
    }
    t.rem_euclid(modulus as i32) as u16
}

fn emit_binary_field_table(
    builder: &mut Builder,
    selector: &[QubitId],
    left: &[QubitId],
    right: &[QubitId],
    out: &[QubitId],
    temps: &[QubitId],
    op: fn(u16, u16, u16) -> u16,
) {
    let mut input = Vec::with_capacity(FIELD_SELECTOR_WIDTH + 2 * FIELD_TEST_WIDTH);
    input.extend_from_slice(selector);
    input.extend_from_slice(left);
    input.extend_from_slice(right);

    for &(selector_value, modulus) in &FIELD_SPECS {
        for left_value in 0..modulus {
            for right_value in 0..modulus {
                let value = op(left_value, right_value, modulus);
                if value == 0 {
                    continue;
                }
                let input_value = u32::from(selector_value)
                    | (u32::from(left_value) << FIELD_SELECTOR_WIDTH)
                    | (u32::from(right_value) << (FIELD_SELECTOR_WIDTH + FIELD_TEST_WIDTH));
                emit_exact_match_flag(builder, &input, input_value, temps);
                emit_scalar_xor(builder, temps[input.len() - 2], value, out);
                unemit_exact_match_flag(builder, &input, input_value, temps);
            }
        }
    }
}

fn emit_unary_field_table(
    builder: &mut Builder,
    selector: &[QubitId],
    input: &[QubitId],
    out: &[QubitId],
    temps: &[QubitId],
    op: fn(u16, u16) -> u16,
) {
    let mut selector_and_input = Vec::with_capacity(FIELD_SELECTOR_WIDTH + FIELD_TEST_WIDTH);
    selector_and_input.extend_from_slice(selector);
    selector_and_input.extend_from_slice(input);

    for &(selector_value, modulus) in &FIELD_SPECS {
        for input_value in 0..modulus {
            let value = op(input_value, modulus);
            if value == 0 {
                continue;
            }
            let match_value =
                u32::from(selector_value) | (u32::from(input_value) << FIELD_SELECTOR_WIDTH);
            emit_exact_match_flag(builder, &selector_and_input, match_value, temps);
            emit_scalar_xor(builder, temps[selector_and_input.len() - 2], value, out);
            unemit_exact_match_flag(builder, &selector_and_input, match_value, temps);
        }
    }
}

fn emit_register_copy(builder: &mut Builder, input: &[QubitId], out: &[QubitId]) {
    for (&control, &target) in input.iter().zip(out) {
        builder.cx(control, target);
    }
}

fn emit_zero_when_denominator_is_zero(
    builder: &mut Builder,
    denominator: &[QubitId],
    numerator: &[QubitId],
    target: &[QubitId],
    temps: &[QubitId],
) {
    emit_exact_match_flag(builder, denominator, 0, temps);
    let zero_flag = temps[denominator.len() - 2];
    for (&control, &target) in numerator.iter().zip(target) {
        builder.ccx(zero_flag, control, target);
    }
    unemit_exact_match_flag(builder, denominator, 0, temps);
}

pub fn build() -> Vec<Op> {
    let mut builder = Builder::new();
    let a = builder.alloc_qubits(WIDTH);
    let b = builder.alloc_qubits(WIDTH);
    let px = builder.alloc_qubits(WIDTH);
    let py = builder.alloc_qubits(WIDTH);
    let pinf = builder.alloc_qubit();
    let qx = builder.alloc_qubits(WIDTH);
    let qy = builder.alloc_qubits(WIDTH);
    let qinf = builder.alloc_qubit();
    let rx = builder.alloc_qubits(WIDTH);
    let ry = builder.alloc_qubits(WIDTH);
    let rinf = builder.alloc_qubit();
    let sum_x = builder.alloc_qubits(WIDTH);
    let sum_y = builder.alloc_qubits(WIDTH);
    let sum_inf = builder.alloc_qubit();
    let double_x = builder.alloc_qubits(WIDTH);
    let double_y = builder.alloc_qubits(WIDTH);
    let double_inf = builder.alloc_qubit();
    let field_selector = builder.alloc_qubits(FIELD_SELECTOR_WIDTH);
    let field_x1 = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_y1 = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_x2 = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_y2 = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_sum = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_den = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_num = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_product = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_den_inv = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_lambda = builder.alloc_qubits(FIELD_TEST_WIDTH);
    let field_lambda_den = builder.alloc_qubits(FIELD_TEST_WIDTH);

    let ax = builder.alloc_qubits(WIDTH);
    let ay = builder.alloc_qubits(WIDTH);
    let ainf = builder.alloc_qubit();
    let bx = builder.alloc_qubits(WIDTH);
    let by = builder.alloc_qubits(WIDTH);
    let binf = builder.alloc_qubit();
    let temps = builder.alloc_qubits(2 * (2 * WIDTH + 1) - 1);
    let field_temps = builder.alloc_qubits(FIELD_SELECTOR_WIDTH + 2 * FIELD_TEST_WIDTH - 1);

    builder.declare_qubit_register(&a);
    builder.declare_qubit_register(&b);
    builder.declare_qubit_register(&px);
    builder.declare_qubit_register(&py);
    builder.declare_qubit_register(&[pinf]);
    builder.declare_qubit_register(&qx);
    builder.declare_qubit_register(&qy);
    builder.declare_qubit_register(&[qinf]);
    builder.declare_qubit_register(&rx);
    builder.declare_qubit_register(&ry);
    builder.declare_qubit_register(&[rinf]);
    builder.declare_qubit_register(&sum_x);
    builder.declare_qubit_register(&sum_y);
    builder.declare_qubit_register(&[sum_inf]);
    builder.declare_qubit_register(&double_x);
    builder.declare_qubit_register(&double_y);
    builder.declare_qubit_register(&[double_inf]);
    builder.declare_qubit_register(&field_selector);
    builder.declare_qubit_register(&field_x1);
    builder.declare_qubit_register(&field_y1);
    builder.declare_qubit_register(&field_x2);
    builder.declare_qubit_register(&field_y2);
    builder.declare_qubit_register(&field_sum);
    builder.declare_qubit_register(&field_den);
    builder.declare_qubit_register(&field_num);
    builder.declare_qubit_register(&field_product);
    builder.declare_qubit_register(&field_den_inv);
    builder.declare_qubit_register(&field_lambda);
    builder.declare_qubit_register(&field_lambda_den);

    emit_scalar_point_table(&mut builder, &a, &px, &py, pinf, &ax, &ay, ainf, &temps);
    emit_scalar_point_table(&mut builder, &b, &qx, &qy, qinf, &bx, &by, binf, &temps);
    emit_point_add_table(
        &mut builder,
        &ax,
        &ay,
        ainf,
        &bx,
        &by,
        binf,
        &rx,
        &ry,
        rinf,
        &temps,
    );
    emit_point_add_table(
        &mut builder,
        &px,
        &py,
        pinf,
        &qx,
        &qy,
        qinf,
        &sum_x,
        &sum_y,
        sum_inf,
        &temps,
    );
    emit_point_double_table(
        &mut builder,
        &px,
        &py,
        pinf,
        &double_x,
        &double_y,
        double_inf,
        &temps,
    );
    emit_scalar_point_table(&mut builder, &b, &qx, &qy, qinf, &bx, &by, binf, &temps);
    emit_scalar_point_table(&mut builder, &a, &px, &py, pinf, &ax, &ay, ainf, &temps);
    emit_binary_field_table(
        &mut builder,
        &field_selector,
        &field_x1,
        &field_x2,
        &field_sum,
        &field_temps,
        test_add,
    );
    emit_binary_field_table(
        &mut builder,
        &field_selector,
        &field_x2,
        &field_x1,
        &field_den,
        &field_temps,
        test_sub,
    );
    emit_binary_field_table(
        &mut builder,
        &field_selector,
        &field_y2,
        &field_y1,
        &field_num,
        &field_temps,
        test_sub,
    );
    emit_binary_field_table(
        &mut builder,
        &field_selector,
        &field_x1,
        &field_y2,
        &field_product,
        &field_temps,
        test_mul,
    );
    emit_unary_field_table(
        &mut builder,
        &field_selector,
        &field_den,
        &field_den_inv,
        &field_temps,
        test_inv,
    );
    emit_binary_field_table(
        &mut builder,
        &field_selector,
        &field_num,
        &field_den_inv,
        &field_lambda,
        &field_temps,
        test_mul,
    );
    emit_register_copy(&mut builder, &field_num, &field_lambda_den);
    emit_zero_when_denominator_is_zero(
        &mut builder,
        &field_den,
        &field_num,
        &field_lambda_den,
        &field_temps,
    );

    builder.ops
}
