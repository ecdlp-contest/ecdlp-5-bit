//! Low-qubit reversible composition for the 5-bit Shor ECDLP variable-base
//! oracle.
//!
//! The circuit still computes the scored oracle over `F_31`:
//! `|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>`.

use crate::circuit::{Op, QubitId};
use crate::ops_io::{OpSink, VecOpSink};

mod builder;
mod field_arithmetic;

use builder::{const_bits, Builder, Signal, FIELD_MODULUS, WIDTH};

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
    qubits.iter().copied().map(Signal::qubit).collect()
}

fn point_signals(point: &PointRegister) -> PointValue {
    PointValue {
        x: qubit_signals(&point.x),
        y: qubit_signals(&point.y),
        inf: Signal::qubit(point.inf),
    }
}

fn point_const(x: u16, y: u16, inf: bool) -> PointValue {
    PointValue {
        x: const_bits(x, WIDTH),
        y: const_bits(y, WIDTH),
        inf: Signal::constant(inf),
    }
}

fn hold_point(builder: &mut Builder<impl OpSink>) -> PointRegister {
    PointRegister {
        x: builder.hold_qubits(WIDTH),
        y: builder.hold_qubits(WIDTH),
        inf: builder.hold_qubits(1)[0],
    }
}

fn release_point(builder: &mut Builder<impl OpSink>, point: PointRegister) {
    builder.release_hold_qubits(point.x);
    builder.release_hold_qubits(point.y);
    builder.release_hold_qubits(vec![point.inf]);
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

fn copy_point(builder: &mut Builder<impl OpSink>, point: PointValue, target: &PointRegister) {
    builder.finish_segment(
        vec![(point.x, target.x.clone()), (point.y, target.y.clone())],
        vec![(point.inf, target.inf)],
    );
}

fn xor_point_into(
    builder: &mut Builder<impl OpSink>,
    source: &PointRegister,
    target: &PointRegister,
) {
    builder.xor_bits_into(&source.x, &target.x);
    builder.xor_bits_into(&source.y, &target.y);
    builder.xor_qubit_into(source.inf, target.inf);
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

fn same_point_signal(builder: &mut Builder<impl OpSink>, point: &PointRegister) -> Signal {
    let y_zero = builder.is_zero(&qubit_signals(&point.y));
    builder.or(Signal::qubit(point.inf), y_zero)
}

fn add_same_point_signal(
    builder: &mut Builder<impl OpSink>,
    left: &PointRegister,
    right: &PointRegister,
) -> Signal {
    let same_x = builder.eq_bits(&qubit_signals(&left.x), &qubit_signals(&right.x));
    let same_y = builder.eq_bits(&qubit_signals(&left.y), &qubit_signals(&right.y));
    let same_xy = builder.and(same_x, same_y);
    let not_left_inf = builder.not(Signal::qubit(left.inf));
    let not_right_inf = builder.not(Signal::qubit(right.inf));
    let neither_inf = builder.and(not_left_inf, not_right_inf);
    builder.and(same_xy, neither_inf)
}

fn inverse_case_signal(
    builder: &mut Builder<impl OpSink>,
    left: &PointRegister,
    right: &PointRegister,
    y_sum: &[QubitId],
) -> Signal {
    let same_x = builder.eq_bits(&qubit_signals(&left.x), &qubit_signals(&right.x));
    let y_sum_zero = builder.is_zero(&qubit_signals(y_sum));
    let same_x_and_neg_y = builder.and(same_x, y_sum_zero);
    let not_left_inf = builder.not(Signal::qubit(left.inf));
    let not_right_inf = builder.not(Signal::qubit(right.inf));
    let neither_inf = builder.and(not_left_inf, not_right_inf);
    builder.and(same_x_and_neg_y, neither_inf)
}

fn point_double_xor(
    builder: &mut Builder<impl OpSink>,
    point: &PointRegister,
    out: &PointRegister,
) {
    let x_squared = builder.hold_qubits(WIDTH);
    let numerator = builder.hold_qubits(WIDTH);
    let denominator = builder.hold_qubits(WIDTH);
    let denominator_inv = builder.hold_qubits(WIDTH);
    let lambda = builder.hold_qubits(WIDTH);
    let lambda_squared = builder.hold_qubits(WIDTH);
    let double_x = builder.hold_qubits(WIDTH);
    let x3 = builder.hold_qubits(WIDTH);
    let x_minus_x3 = builder.hold_qubits(WIDTH);
    let lambda_times_delta = builder.hold_qubits(WIDTH);
    let y3 = builder.hold_qubits(WIDTH);

    let compute_tape = builder.record(|builder| {
        builder.xor_mul_mod_into(&point.x, &point.x, &x_squared);
        builder.xor_mul_const_mod_into(&x_squared, 3, &numerator);
        builder.xor_mul_const_mod_into(&point.y, 2, &denominator);
        builder.xor_inv_mod_into(&denominator, &denominator_inv);
        builder.xor_mul_mod_into(&numerator, &denominator_inv, &lambda);
        builder.xor_mul_mod_into(&lambda, &lambda, &lambda_squared);
        builder.xor_mul_const_mod_into(&point.x, 2, &double_x);
        builder.xor_sub_mod_into(&lambda_squared, &double_x, &x3);
        builder.xor_sub_mod_into(&point.x, &x3, &x_minus_x3);
        builder.xor_mul_mod_into(&lambda, &x_minus_x3, &lambda_times_delta);
        builder.xor_sub_mod_into(&lambda_times_delta, &point.y, &y3);
    });

    let formula = PointValue {
        x: qubit_signals(&x3),
        y: qubit_signals(&y3),
        inf: Signal::constant(false),
    };
    let invalid_double = same_point_signal(builder, point);
    let selected = mux_point(builder, invalid_double, &point_const(0, 0, true), &formula);
    copy_point(builder, selected, out);

    builder.append_reverse_tape(&compute_tape);
    for field in [
        x_squared,
        numerator,
        denominator,
        denominator_inv,
        lambda,
        lambda_squared,
        double_x,
        x3,
        x_minus_x3,
        lambda_times_delta,
        y3,
    ] {
        builder.release_hold_qubits(field);
    }
}

fn point_add_xor(
    builder: &mut Builder<impl OpSink>,
    left: &PointRegister,
    right: &PointRegister,
    out: &PointRegister,
) {
    let y_sum = builder.hold_qubits(WIDTH);
    let add_num = builder.hold_qubits(WIDTH);
    let add_den = builder.hold_qubits(WIDTH);
    let add_den_inv = builder.hold_qubits(WIDTH);
    let lambda_add = builder.hold_qubits(WIDTH);
    let x_squared = builder.hold_qubits(WIDTH);
    let double_num = builder.hold_qubits(WIDTH);
    let double_den = builder.hold_qubits(WIDTH);
    let double_den_inv = builder.hold_qubits(WIDTH);
    let lambda_double = builder.hold_qubits(WIDTH);
    let lambda = builder.hold_qubits(WIDTH);
    let lambda_squared = builder.hold_qubits(WIDTH);
    let x_minus_left = builder.hold_qubits(WIDTH);
    let x3 = builder.hold_qubits(WIDTH);
    let left_x_minus_x3 = builder.hold_qubits(WIDTH);
    let lambda_times_delta = builder.hold_qubits(WIDTH);
    let y3 = builder.hold_qubits(WIDTH);

    let compute_tape = builder.record(|builder| {
        builder.xor_add_mod_into(&left.y, &right.y, &y_sum);
        builder.xor_sub_mod_into(&right.y, &left.y, &add_num);
        builder.xor_sub_mod_into(&right.x, &left.x, &add_den);
        builder.xor_inv_mod_into(&add_den, &add_den_inv);
        builder.xor_mul_mod_into(&add_num, &add_den_inv, &lambda_add);

        builder.xor_mul_mod_into(&left.x, &left.x, &x_squared);
        builder.xor_mul_const_mod_into(&x_squared, 3, &double_num);
        builder.xor_mul_const_mod_into(&left.y, 2, &double_den);
        builder.xor_inv_mod_into(&double_den, &double_den_inv);
        builder.xor_mul_mod_into(&double_num, &double_den_inv, &lambda_double);

        let same_point = add_same_point_signal(builder, left, right);
        let lambda_bits = builder.mux_bits(
            same_point,
            &qubit_signals(&lambda_double),
            &qubit_signals(&lambda_add),
        );
        builder.finish_segment(vec![(lambda_bits, lambda.clone())], Vec::new());

        builder.xor_mul_mod_into(&lambda, &lambda, &lambda_squared);
        builder.xor_sub_mod_into(&lambda_squared, &left.x, &x_minus_left);
        builder.xor_sub_mod_into(&x_minus_left, &right.x, &x3);
        builder.xor_sub_mod_into(&left.x, &x3, &left_x_minus_x3);
        builder.xor_mul_mod_into(&lambda, &left_x_minus_x3, &lambda_times_delta);
        builder.xor_sub_mod_into(&lambda_times_delta, &left.y, &y3);
    });

    let formula = PointValue {
        x: qubit_signals(&x3),
        y: qubit_signals(&y3),
        inf: Signal::constant(false),
    };
    let inverse_case = inverse_case_signal(builder, left, right, &y_sum);
    let after_inverse = mux_point(builder, inverse_case, &point_const(0, 0, true), &formula);
    let after_right_inf = mux_point(
        builder,
        Signal::qubit(right.inf),
        &point_signals(left),
        &after_inverse,
    );
    let selected = mux_point(
        builder,
        Signal::qubit(left.inf),
        &point_signals(right),
        &after_right_inf,
    );
    copy_point(builder, selected, out);

    builder.append_reverse_tape(&compute_tape);
    for field in [
        y_sum,
        add_num,
        add_den,
        add_den_inv,
        lambda_add,
        x_squared,
        double_num,
        double_den,
        double_den_inv,
        lambda_double,
        lambda,
        lambda_squared,
        x_minus_left,
        x3,
        left_x_minus_x3,
        lambda_times_delta,
        y3,
    ] {
        builder.release_hold_qubits(field);
    }
}

fn point_neg_xor(builder: &mut Builder<impl OpSink>, point: &PointRegister, out: &PointRegister) {
    builder.xor_bits_into(&point.x, &out.x);
    let neg_y = builder.sub_mod(
        &const_bits(0, WIDTH),
        &qubit_signals(&point.y),
        FIELD_MODULUS,
    );
    builder.finish_segment(
        vec![(neg_y, out.y.clone())],
        vec![(Signal::qubit(point.inf), out.inf)],
    );
}

fn point_power_of_two_xor(
    builder: &mut Builder<impl OpSink>,
    point: &PointRegister,
    bit: usize,
    out: &PointRegister,
) {
    if bit == 0 {
        xor_point_into(builder, point, out);
        return;
    }

    let intermediates: Vec<PointRegister> = (0..bit).map(|_| hold_point(builder)).collect();
    let compute_tape = builder.record(|builder| {
        point_double_xor(builder, point, &intermediates[0]);
        for index in 1..bit {
            point_double_xor(builder, &intermediates[index - 1], &intermediates[index]);
        }
    });
    xor_point_into(builder, &intermediates[bit - 1], out);
    builder.append_reverse_tape(&compute_tape);
    for point in intermediates {
        release_point(builder, point);
    }
}

fn xor_selected_point(
    builder: &mut Builder<impl OpSink>,
    selector: QubitId,
    when_true: &PointRegister,
    when_false: &PointRegister,
    target: &PointRegister,
) {
    let selected = mux_point(
        builder,
        Signal::qubit(selector),
        &point_signals(when_true),
        &point_signals(when_false),
    );
    copy_point(builder, selected, target);
}

fn controlled_add_assign(
    builder: &mut Builder<impl OpSink>,
    acc: &PointRegister,
    addend: &PointRegister,
    selector: QubitId,
) {
    let sum = hold_point(builder);
    let tmp = hold_point(builder);
    let neg_addend = hold_point(builder);

    point_add_xor(builder, acc, addend, &sum);
    xor_selected_point(builder, selector, &sum, acc, &tmp);
    point_add_xor(builder, acc, addend, &sum);

    swap_points(builder, acc, &tmp);

    point_neg_xor(builder, addend, &neg_addend);
    point_add_xor(builder, acc, &neg_addend, &sum);
    xor_selected_point(builder, selector, &sum, acc, &tmp);
    point_add_xor(builder, acc, &neg_addend, &sum);
    point_neg_xor(builder, addend, &neg_addend);

    release_point(builder, neg_addend);
    release_point(builder, tmp);
    release_point(builder, sum);
}

fn scalar_mul_into(
    builder: &mut Builder<impl OpSink>,
    scalar: &[QubitId],
    point: &PointRegister,
    out: &PointRegister,
    multiple: &PointRegister,
) {
    builder.toggle(out.inf);
    for bit in 0..WIDTH {
        point_power_of_two_xor(builder, point, bit, multiple);
        controlled_add_assign(builder, out, multiple, scalar[bit]);
        point_power_of_two_xor(builder, point, bit, multiple);
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

    let p = PointRegister {
        x: px,
        y: py,
        inf: pinf,
    };
    let q = PointRegister {
        x: qx,
        y: qy,
        inf: qinf,
    };
    let r = PointRegister {
        x: rx,
        y: ry,
        inf: rinf,
    };

    let a_hold = hold_point(&mut builder);
    let b_hold = hold_point(&mut builder);
    let multiple = hold_point(&mut builder);

    let a_tape = builder.record(|builder| {
        scalar_mul_into(builder, &a, &p, &a_hold, &multiple);
    });
    let b_tape = builder.record(|builder| {
        scalar_mul_into(builder, &b, &q, &b_hold, &multiple);
    });

    point_add_xor(&mut builder, &a_hold, &b_hold, &r);

    builder.append_reverse_tape(&b_tape);
    builder.append_reverse_tape(&a_tape);

    release_point(&mut builder, multiple);
    release_point(&mut builder, b_hold);
    release_point(&mut builder, a_hold);

    builder.finish_sink()
}
