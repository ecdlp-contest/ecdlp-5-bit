//! Trusted composition for the 5-bit Shor ECDLP variable-base oracle.
//!
//! Contest submissions may optimize this module and its `field_arithmetic`
//! submodule. The contract is still the Shor oracle over `F_31`; implementations
//! should use reversible field arithmetic rather than lookup tables.

use crate::circuit::{Op, QubitId};
use crate::ops_io::{OpSink, VecOpSink};

mod field_arithmetic;

use field_arithmetic::{bit_at, const_bits, Builder, Signal, FIELD_MODULUS, WIDTH};

#[derive(Clone)]
struct PointValue {
    x: Vec<Signal>,
    y: Vec<Signal>,
    inf: Signal,
}

#[derive(Clone)]
struct PointRegister {
    x: Vec<QubitId>,
    y: Vec<QubitId>,
    inf: QubitId,
}

fn qubit_signals(qubits: &[QubitId]) -> Vec<Signal> {
    qubits.iter().copied().map(Signal::Qubit).collect()
}

fn held_point(register: &PointRegister) -> PointValue {
    PointValue {
        x: qubit_signals(&register.x),
        y: qubit_signals(&register.y),
        inf: Signal::Qubit(register.inf),
    }
}

fn point_const(x: u16, y: u16, inf: bool) -> PointValue {
    PointValue {
        x: const_bits(x, WIDTH),
        y: const_bits(y, WIDTH),
        inf: Signal::Const(inf),
    }
}

fn mux_point(
    builder: &mut Builder<impl OpSink>,
    selector: Signal,
    when_true: &PointValue,
    when_false: &PointValue,
) -> PointValue {
    PointValue {
        x: builder.mux_bits(selector.clone(), &when_true.x, &when_false.x),
        y: builder.mux_bits(selector.clone(), &when_true.y, &when_false.y),
        inf: builder.mux(selector, when_true.inf.clone(), when_false.inf.clone()),
    }
}

fn point_add(
    builder: &mut Builder<impl OpSink>,
    left: &PointValue,
    right: &PointValue,
) -> PointValue {
    let same_x = builder.eq_bits(&left.x, &right.x);
    let same_y = builder.eq_bits(&left.y, &right.y);
    let not_left_inf = builder.not(left.inf.clone());
    let not_right_inf = builder.not(right.inf.clone());
    let neither_inf = builder.and(not_left_inf, not_right_inf);

    let y_sum = builder.add_mod(&left.y, &right.y, FIELD_MODULUS);
    let y_sum_zero = builder.is_zero(&y_sum);
    let same_x_and_neg_y = builder.and(same_x.clone(), y_sum_zero);
    let inverse_case = builder.and(same_x_and_neg_y, neither_inf.clone());

    let same_point_xy = builder.and(same_x, same_y);
    let same_point = builder.and(same_point_xy, neither_inf);

    let add_num = builder.sub_mod(&right.y, &left.y, FIELD_MODULUS);
    let add_den = builder.sub_mod(&right.x, &left.x, FIELD_MODULUS);
    let add_den_inv = builder.inv_mod(&add_den, FIELD_MODULUS);
    let lambda_add = builder.mul_mod(&add_num, &add_den_inv, FIELD_MODULUS);

    let x_squared = builder.mul_mod(&left.x, &left.x, FIELD_MODULUS);
    let double_num = builder.mul_const_mod(&x_squared, 3, FIELD_MODULUS);
    let double_den = builder.mul_const_mod(&left.y, 2, FIELD_MODULUS);
    let double_den_inv = builder.inv_mod(&double_den, FIELD_MODULUS);
    let lambda_double = builder.mul_mod(&double_num, &double_den_inv, FIELD_MODULUS);
    let lambda = builder.mux_bits(same_point, &lambda_double, &lambda_add);

    let lambda_squared = builder.mul_mod(&lambda, &lambda, FIELD_MODULUS);
    let x_minus_left = builder.sub_mod(&lambda_squared, &left.x, FIELD_MODULUS);
    let x3 = builder.sub_mod(&x_minus_left, &right.x, FIELD_MODULUS);
    let left_x_minus_x3 = builder.sub_mod(&left.x, &x3, FIELD_MODULUS);
    let lambda_times_delta = builder.mul_mod(&lambda, &left_x_minus_x3, FIELD_MODULUS);
    let y3 = builder.sub_mod(&lambda_times_delta, &left.y, FIELD_MODULUS);

    let formula = PointValue {
        x: x3,
        y: y3,
        inf: Signal::Const(false),
    };
    let infinity = point_const(0, 0, true);
    let after_inverse = mux_point(builder, inverse_case, &infinity, &formula);
    let after_right_inf = mux_point(builder, right.inf.clone(), left, &after_inverse);
    mux_point(builder, left.inf.clone(), right, &after_right_inf)
}

fn point_double(builder: &mut Builder<impl OpSink>, point: &PointValue) -> PointValue {
    let y_zero = builder.is_zero(&point.y);
    let x_squared = builder.mul_mod(&point.x, &point.x, FIELD_MODULUS);
    let numerator = builder.mul_const_mod(&x_squared, 3, FIELD_MODULUS);
    let denominator = builder.mul_const_mod(&point.y, 2, FIELD_MODULUS);
    let denominator_inv = builder.inv_mod(&denominator, FIELD_MODULUS);
    let lambda = builder.mul_mod(&numerator, &denominator_inv, FIELD_MODULUS);
    let lambda_squared = builder.mul_mod(&lambda, &lambda, FIELD_MODULUS);
    let double_x = builder.mul_const_mod(&point.x, 2, FIELD_MODULUS);
    let x3 = builder.sub_mod(&lambda_squared, &double_x, FIELD_MODULUS);
    let x_minus_x3 = builder.sub_mod(&point.x, &x3, FIELD_MODULUS);
    let lambda_times_delta = builder.mul_mod(&lambda, &x_minus_x3, FIELD_MODULUS);
    let y3 = builder.sub_mod(&lambda_times_delta, &point.y, FIELD_MODULUS);

    let formula = PointValue {
        x: x3,
        y: y3,
        inf: Signal::Const(false),
    };
    let infinity = point_const(0, 0, true);
    let invalid_double = builder.or(point.inf.clone(), y_zero);
    mux_point(builder, invalid_double, &infinity, &formula)
}

fn point_neg(builder: &mut Builder<impl OpSink>, point: &PointValue) -> PointValue {
    let zero = const_bits(0, WIDTH);
    PointValue {
        x: point.x.clone(),
        y: builder.sub_mod(&zero, &point.y, FIELD_MODULUS),
        inf: point.inf.clone(),
    }
}

fn point_power_of_two(
    builder: &mut Builder<impl OpSink>,
    point: &PointValue,
    bit: usize,
) -> PointValue {
    let mut cur = point.clone();
    for _ in 0..bit {
        cur = point_double(builder, &cur);
    }
    cur
}

fn copy_point(builder: &mut Builder<impl OpSink>, point: PointValue, target: &PointRegister) {
    builder.finish_segment(
        vec![(point.x, target.x.clone()), (point.y, target.y.clone())],
        vec![(point.inf, target.inf)],
    );
}

fn swap_points(builder: &mut Builder<impl OpSink>, left: &PointRegister, right: &PointRegister) {
    for (&a, &b) in left.x.iter().zip(&right.x) {
        builder.swap(a, b);
    }
    for (&a, &b) in left.y.iter().zip(&right.y) {
        builder.swap(a, b);
    }
    builder.swap(left.inf, right.inf);
}

fn controlled_add_assign(
    builder: &mut Builder<impl OpSink>,
    acc: &PointRegister,
    addend: &PointRegister,
    selector: Signal,
    tmp: &PointRegister,
) {
    let acc_before = held_point(acc);
    let addend_value = held_point(addend);
    let sum = point_add(builder, &acc_before, &addend_value);
    let next = mux_point(builder, selector.clone(), &sum, &acc_before);
    copy_point(builder, next, tmp);

    swap_points(builder, acc, tmp);

    let acc_after = held_point(acc);
    let addend_value = held_point(addend);
    let neg_addend = point_neg(builder, &addend_value);
    let previous_if_added = point_add(builder, &acc_after, &neg_addend);
    let previous = mux_point(builder, selector, &previous_if_added, &acc_after);
    copy_point(builder, previous, tmp);
}

fn scalar_mul_into(
    builder: &mut Builder<impl OpSink>,
    scalar: &[Signal],
    point: &PointValue,
    out: &PointRegister,
    multiple: &PointRegister,
    tmp: &PointRegister,
) {
    builder.finish_segment(Vec::new(), vec![(Signal::Const(true), out.inf)]);
    for bit in 0..WIDTH {
        let multiple_value = point_power_of_two(builder, point, bit);
        copy_point(builder, multiple_value.clone(), multiple);
        controlled_add_assign(builder, out, multiple, bit_at(scalar, bit), tmp);
        copy_point(builder, multiple_value, multiple);
    }
}

pub fn build() -> Vec<Op> {
    build_into(VecOpSink::default()).into_ops()
}

pub fn build_into<S: OpSink>(sink: S) -> S {
    let mut builder = Builder::with_sink(sink);
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

    let p = PointValue {
        x: qubit_signals(&px),
        y: qubit_signals(&py),
        inf: Signal::Qubit(pinf),
    };
    let q = PointValue {
        x: qubit_signals(&qx),
        y: qubit_signals(&qy),
        inf: Signal::Qubit(qinf),
    };

    let a_hold = PointRegister {
        x: builder.hold_qubits(WIDTH),
        y: builder.hold_qubits(WIDTH),
        inf: builder.hold_qubits(1)[0],
    };
    let b_hold = PointRegister {
        x: builder.hold_qubits(WIDTH),
        y: builder.hold_qubits(WIDTH),
        inf: builder.hold_qubits(1)[0],
    };
    let multiple = PointRegister {
        x: builder.hold_qubits(WIDTH),
        y: builder.hold_qubits(WIDTH),
        inf: builder.hold_qubits(1)[0],
    };
    let tmp = PointRegister {
        x: builder.hold_qubits(WIDTH),
        y: builder.hold_qubits(WIDTH),
        inf: builder.hold_qubits(1)[0],
    };

    let a_signals = qubit_signals(&a);
    let a_tape = builder.record(|builder| {
        scalar_mul_into(builder, &a_signals, &p, &a_hold, &multiple, &tmp);
    });

    let b_signals = qubit_signals(&b);
    let b_tape = builder.record(|builder| {
        scalar_mul_into(builder, &b_signals, &q, &b_hold, &multiple, &tmp);
    });

    let a_point = held_point(&a_hold);
    let b_point = held_point(&b_hold);
    let r = point_add(&mut builder, &a_point, &b_point);
    let r_reg = PointRegister {
        x: rx,
        y: ry,
        inf: rinf,
    };
    copy_point(&mut builder, r, &r_reg);

    builder.append_reverse_tape(&b_tape);
    builder.release_hold_qubits(b_hold.x);
    builder.release_hold_qubits(b_hold.y);
    builder.release_hold_qubits(vec![b_hold.inf]);

    builder.append_reverse_tape(&a_tape);
    builder.release_hold_qubits(a_hold.x);
    builder.release_hold_qubits(a_hold.y);
    builder.release_hold_qubits(vec![a_hold.inf]);
    builder.release_hold_qubits(multiple.x);
    builder.release_hold_qubits(multiple.y);
    builder.release_hold_qubits(vec![multiple.inf]);
    builder.release_hold_qubits(tmp.x);
    builder.release_hold_qubits(tmp.y);
    builder.release_hold_qubits(vec![tmp.inf]);

    builder.finish_sink()
}
