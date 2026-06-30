use crate::circuit::{Op, OperationType, QubitId, RegisterId};
use crate::ops_io::OpSink;
use std::rc::Rc;

use super::field_arithmetic;

pub(crate) const WIDTH: usize = 5;
pub(crate) const FIELD_MODULUS: u16 = 31;

pub(crate) fn bit_at(bits: &[Signal], index: usize) -> Signal {
    bits.get(index).cloned().unwrap_or(Signal::constant(false))
}

pub(crate) fn const_bits(value: u16, width: usize) -> Vec<Signal> {
    (0..width)
        .map(|bit| Signal::constant(((value >> bit) & 1) != 0))
        .collect()
}

fn qubit_signals(qubits: &[QubitId]) -> Vec<Signal> {
    qubits.iter().copied().map(Signal::qubit).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Signal(SignalKind);

#[derive(Clone, Debug, PartialEq, Eq)]
enum SignalKind {
    Const(bool),
    Qubit(QubitId),
    Expr(Rc<Expr>),
}

impl Signal {
    pub(crate) fn constant(value: bool) -> Self {
        Self(SignalKind::Const(value))
    }

    pub(crate) fn qubit(qubit: QubitId) -> Self {
        Self(SignalKind::Qubit(qubit))
    }

    fn expr(expr: Rc<Expr>) -> Self {
        Self(SignalKind::Expr(expr))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Expr {
    Not(Signal),
    Xor(Signal, Signal),
    And(Signal, Signal),
}

#[derive(Clone, Copy)]
pub(crate) enum BinaryFieldOperation {
    Add,
    Sub,
    Mul,
}

#[derive(Clone, Copy)]
pub(crate) enum UnaryFieldOperation {
    MulConst(u16),
    Inv,
}

#[derive(Clone, Copy)]
pub(crate) struct FieldInput<'a> {
    bits: &'a [QubitId],
}

#[derive(Clone, Copy)]
pub(crate) struct FieldOutput<'a> {
    bits: &'a [QubitId],
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct FieldTargetBit {
    qubit: QubitId,
}

pub(crate) struct FieldEmitter<'a, S: OpSink> {
    builder: &'a mut Builder<S>,
}

pub(crate) struct Builder<S: OpSink> {
    sink: S,
    recording: Vec<Vec<Op>>,
    free_scratch: Vec<QubitId>,
    next_qubit: u64,
    next_register: u64,
}

#[allow(dead_code)]
impl<'a, S: OpSink> FieldEmitter<'a, S> {
    pub(crate) fn input_bits(&self, input: FieldInput<'_>) -> Vec<Signal> {
        qubit_signals(input.bits)
    }

    pub(crate) fn target_bit(&self, output: FieldOutput<'_>, bit: usize) -> Option<FieldTargetBit> {
        output
            .bits
            .get(bit)
            .copied()
            .map(|qubit| FieldTargetBit { qubit })
    }

    pub(crate) fn xor_signal_into(&mut self, signal: &Signal, target: FieldTargetBit) {
        self.builder.xor_signal_into(signal, target.qubit);
    }

    pub(crate) fn xor_binary_mod_into(
        &mut self,
        operation: BinaryFieldOperation,
        left: FieldInput<'_>,
        right: FieldInput<'_>,
        target: FieldOutput<'_>,
    ) {
        self.builder
            .xor_field_binary_into(operation, left.bits, right.bits, target.bits);
    }

    pub(crate) fn xor_unary_mod_into(
        &mut self,
        operation: UnaryFieldOperation,
        input: FieldInput<'_>,
        target: FieldOutput<'_>,
    ) {
        self.builder
            .xor_field_unary_into(operation, input.bits, target.bits);
    }
}

impl<S: OpSink> Builder<S> {
    pub(crate) fn with_sink(sink: S) -> Self {
        Self {
            sink,
            recording: Vec::new(),
            free_scratch: Vec::new(),
            next_qubit: 0,
            next_register: 0,
        }
    }

    pub(crate) fn alloc_qubit(&mut self) -> QubitId {
        let qubit = QubitId(self.next_qubit);
        self.next_qubit += 1;
        qubit
    }

    pub(crate) fn alloc_qubits(&mut self, n: usize) -> Vec<QubitId> {
        (0..n).map(|_| self.alloc_qubit()).collect()
    }

    pub(crate) fn declare_qubit_register(&mut self, qubits: &[QubitId]) {
        let register = RegisterId(self.next_register);
        self.next_register += 1;
        for &qubit in qubits {
            let mut op = Op::empty();
            op.kind = OperationType::AppendToRegister;
            op.q_target = qubit;
            op.r_target = register;
            op.validate();
            self.push_op(op);
        }
        let mut op = Op::empty();
        op.kind = OperationType::Register;
        op.r_target = register;
        op.validate();
        self.push_op(op);
    }

    fn push_op(&mut self, op: Op) {
        op.validate();
        for recording in &mut self.recording {
            recording.push(op);
        }
        self.sink.push_op(op);
    }

    fn x_gate(target: QubitId) -> Op {
        let mut op = Op::empty();
        op.kind = OperationType::X;
        op.q_target = target;
        op
    }

    fn cx_gate(control: QubitId, target: QubitId) -> Op {
        let mut op = Op::empty();
        op.kind = OperationType::CX;
        op.q_control1 = control;
        op.q_target = target;
        op
    }

    fn ccx_gate(control1: QubitId, control2: QubitId, target: QubitId) -> Op {
        let mut op = Op::empty();
        op.kind = OperationType::CCX;
        op.q_control1 = control1;
        op.q_control2 = control2;
        op.q_target = target;
        op
    }

    fn swap_gate(left: QubitId, right: QubitId) -> Op {
        let mut op = Op::empty();
        op.kind = OperationType::Swap;
        op.q_control1 = left;
        op.q_target = right;
        op
    }

    fn push_x(&mut self, target: QubitId) {
        self.push_op(Self::x_gate(target));
    }

    fn push_cx(&mut self, control: QubitId, target: QubitId) {
        if control != target {
            self.push_op(Self::cx_gate(control, target));
        }
    }

    fn push_ccx(&mut self, control1: QubitId, control2: QubitId, target: QubitId) {
        self.push_op(Self::ccx_gate(control1, control2, target));
    }

    pub(crate) fn swap(&mut self, left: QubitId, right: QubitId) {
        if left != right {
            self.push_op(Self::swap_gate(left, right));
        }
    }

    pub(crate) fn toggle(&mut self, target: QubitId) {
        self.push_x(target);
    }

    pub(crate) fn xor_qubit_into(&mut self, control: QubitId, target: QubitId) {
        self.push_cx(control, target);
    }

    pub(crate) fn xor_bits_into(&mut self, source: &[QubitId], target: &[QubitId]) {
        for (&source, &target) in source.iter().zip(target) {
            self.push_cx(source, target);
        }
    }

    pub(crate) fn xor_add_mod_into(
        &mut self,
        left: &[QubitId],
        right: &[QubitId],
        target: &[QubitId],
    ) {
        let mut emitter = FieldEmitter { builder: self };
        field_arithmetic::xor_add_mod_into(
            &mut emitter,
            FieldInput { bits: left },
            FieldInput { bits: right },
            FieldOutput { bits: target },
        );
    }

    pub(crate) fn xor_sub_mod_into(
        &mut self,
        left: &[QubitId],
        right: &[QubitId],
        target: &[QubitId],
    ) {
        let mut emitter = FieldEmitter { builder: self };
        field_arithmetic::xor_sub_mod_into(
            &mut emitter,
            FieldInput { bits: left },
            FieldInput { bits: right },
            FieldOutput { bits: target },
        );
    }

    pub(crate) fn xor_mul_mod_into(
        &mut self,
        left: &[QubitId],
        right: &[QubitId],
        target: &[QubitId],
    ) {
        let mut emitter = FieldEmitter { builder: self };
        field_arithmetic::xor_mul_mod_into(
            &mut emitter,
            FieldInput { bits: left },
            FieldInput { bits: right },
            FieldOutput { bits: target },
        );
    }

    pub(crate) fn xor_mul_const_mod_into(
        &mut self,
        input: &[QubitId],
        factor: u16,
        target: &[QubitId],
    ) {
        let mut emitter = FieldEmitter { builder: self };
        field_arithmetic::xor_mul_const_mod_into(
            &mut emitter,
            FieldInput { bits: input },
            factor,
            FieldOutput { bits: target },
        );
    }

    pub(crate) fn xor_inv_mod_into(&mut self, input: &[QubitId], target: &[QubitId]) {
        let mut emitter = FieldEmitter { builder: self };
        field_arithmetic::xor_inv_mod_into(
            &mut emitter,
            FieldInput { bits: input },
            FieldOutput { bits: target },
        );
    }

    fn xor_field_binary_into(
        &mut self,
        operation: BinaryFieldOperation,
        left: &[QubitId],
        right: &[QubitId],
        target: &[QubitId],
    ) {
        match operation {
            BinaryFieldOperation::Add => {
                self.xor_add_mod31_signals_into(&qubit_signals(left), &qubit_signals(right), target)
            }
            BinaryFieldOperation::Sub => {
                let right = qubit_signals(right);
                let right_complement = self.complement_field_bits(&right);
                self.xor_add_mod31_signals_into(&qubit_signals(left), &right_complement, target);
            }
            BinaryFieldOperation::Mul => self.xor_mul_mod31_into(left, right, target),
        }
    }

    fn xor_field_unary_into(
        &mut self,
        operation: UnaryFieldOperation,
        input: &[QubitId],
        target: &[QubitId],
    ) {
        match operation {
            UnaryFieldOperation::MulConst(factor) => {
                let input = qubit_signals(input);
                let result = self.mul_const_mod31_bits(&input, factor);
                self.copy_bits_ops(&result, target);
            }
            UnaryFieldOperation::Inv => self.xor_inv_mod31_into(input, target),
        }
    }

    pub(crate) fn record<F>(&mut self, f: F) -> Vec<Op>
    where
        F: FnOnce(&mut Self),
    {
        self.recording.push(Vec::new());
        f(self);
        self.recording.pop().unwrap()
    }

    pub(crate) fn append_reverse_tape(&mut self, tape: &[Op]) {
        for &op in tape.iter().rev() {
            self.push_op(op);
        }
    }

    fn scratch(&mut self) -> QubitId {
        self.free_scratch
            .pop()
            .unwrap_or_else(|| self.alloc_qubit())
    }

    fn release_scratch(&mut self, qubit: QubitId) {
        self.free_scratch.push(qubit);
    }

    pub(crate) fn hold_qubits(&mut self, n: usize) -> Vec<QubitId> {
        (0..n)
            .map(|_| {
                self.free_scratch
                    .pop()
                    .unwrap_or_else(|| self.alloc_qubit())
            })
            .collect()
    }

    pub(crate) fn release_hold_qubits(&mut self, qubits: Vec<QubitId>) {
        self.free_scratch.extend(qubits);
    }

    pub(crate) fn not(&mut self, signal: Signal) -> Signal {
        match signal {
            Signal(SignalKind::Const(value)) => Signal::constant(!value),
            Signal(SignalKind::Expr(expr)) => {
                if let Expr::Not(inner) = expr.as_ref() {
                    inner.clone()
                } else {
                    Signal::expr(Rc::new(Expr::Not(Signal::expr(expr))))
                }
            }
            value => Signal::expr(Rc::new(Expr::Not(value))),
        }
    }

    fn same_signal(left: &Signal, right: &Signal) -> bool {
        match (&left.0, &right.0) {
            (SignalKind::Const(a), SignalKind::Const(b)) => a == b,
            (SignalKind::Qubit(a), SignalKind::Qubit(b)) => a == b,
            (SignalKind::Expr(a), SignalKind::Expr(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    fn xor(&mut self, left: Signal, right: Signal) -> Signal {
        match (left, right) {
            (Signal(SignalKind::Const(a)), Signal(SignalKind::Const(b))) => Signal::constant(a ^ b),
            (Signal(SignalKind::Const(false)), value)
            | (value, Signal(SignalKind::Const(false))) => value,
            (Signal(SignalKind::Const(true)), value) | (value, Signal(SignalKind::Const(true))) => {
                self.not(value)
            }
            (Signal(SignalKind::Qubit(a)), Signal(SignalKind::Qubit(b))) if a == b => {
                Signal::constant(false)
            }
            (a, b) if Self::same_signal(&a, &b) => Signal::constant(false),
            (a, b) => Signal::expr(Rc::new(Expr::Xor(a, b))),
        }
    }

    pub(crate) fn and(&mut self, left: Signal, right: Signal) -> Signal {
        match (left, right) {
            (Signal(SignalKind::Const(a)), Signal(SignalKind::Const(b))) => Signal::constant(a & b),
            (Signal(SignalKind::Const(false)), _) | (_, Signal(SignalKind::Const(false))) => {
                Signal::constant(false)
            }
            (Signal(SignalKind::Const(true)), value) | (value, Signal(SignalKind::Const(true))) => {
                value
            }
            (Signal(SignalKind::Qubit(a)), Signal(SignalKind::Qubit(b))) if a == b => {
                Signal::qubit(a)
            }
            (a, b) if Self::same_signal(&a, &b) => a,
            (a, b) => Signal::expr(Rc::new(Expr::And(a, b))),
        }
    }

    pub(crate) fn or(&mut self, left: Signal, right: Signal) -> Signal {
        let either = self.xor(left.clone(), right.clone());
        let both = self.and(left, right);
        self.xor(either, both)
    }

    pub(crate) fn bit_at(bits: &[Signal], index: usize) -> Signal {
        bit_at(bits, index)
    }

    fn and_many(&mut self, inputs: &[Signal]) -> Signal {
        let mut acc = Signal::constant(true);
        for input in inputs {
            acc = self.and(acc, input.clone());
        }
        acc
    }

    pub(crate) fn eq_bits(&mut self, left: &[Signal], right: &[Signal]) -> Signal {
        let width = left.len().max(right.len());
        let mut terms = Vec::with_capacity(width);
        for bit in 0..width {
            let diff = self.xor(Self::bit_at(left, bit), Self::bit_at(right, bit));
            let same = self.not(diff);
            terms.push(same);
        }
        self.and_many(&terms)
    }

    fn eq_const(&mut self, bits: &[Signal], value: u16) -> Signal {
        let mut terms = Vec::with_capacity(bits.len());
        for (bit, signal) in bits.iter().enumerate() {
            if ((value >> bit) & 1) != 0 {
                terms.push(signal.clone());
            } else {
                terms.push(self.not(signal.clone()));
            }
        }
        self.and_many(&terms)
    }

    pub(crate) fn is_zero(&mut self, bits: &[Signal]) -> Signal {
        self.eq_const(bits, 0)
    }

    pub(crate) fn mux(
        &mut self,
        selector: Signal,
        when_true: Signal,
        when_false: Signal,
    ) -> Signal {
        match selector {
            Signal(SignalKind::Const(true)) => when_true,
            Signal(SignalKind::Const(false)) => when_false,
            _ if Self::same_signal(&when_true, &when_false) => when_true,
            _ => {
                let diff = self.xor(when_true, when_false.clone());
                let selected_diff = self.and(selector, diff);
                self.xor(when_false, selected_diff)
            }
        }
    }

    pub(crate) fn mux_bits(
        &mut self,
        selector: Signal,
        when_true: &[Signal],
        when_false: &[Signal],
    ) -> Vec<Signal> {
        let width = when_true.len().max(when_false.len());
        (0..width)
            .map(|bit| {
                self.mux(
                    selector.clone(),
                    Self::bit_at(when_true, bit),
                    Self::bit_at(when_false, bit),
                )
            })
            .collect()
    }

    pub(crate) fn sub_mod(
        &mut self,
        left: &[Signal],
        right: &[Signal],
        modulus: u16,
    ) -> Vec<Signal> {
        debug_assert_eq!(modulus, FIELD_MODULUS);
        self.sub_mod31_bits(left, right)
    }

    fn add_unsigned_bits(
        &mut self,
        left: &[Signal],
        right: &[Signal],
        width: usize,
    ) -> Vec<Signal> {
        let mut out = Vec::with_capacity(width + 1);
        let mut carry = Signal::constant(false);
        for bit in 0..width {
            let a = Self::bit_at(left, bit);
            let b = Self::bit_at(right, bit);
            let a_xor_b = self.xor(a.clone(), b.clone());
            let sum = self.xor(a_xor_b.clone(), carry.clone());
            let generate = self.and(a, b);
            let propagate = self.and(carry, a_xor_b);
            carry = self.xor(generate, propagate);
            out.push(sum);
        }
        out.push(carry);
        out
    }

    fn add_const_wrapping_bits(
        &mut self,
        bits: &[Signal],
        constant: u16,
        width: usize,
    ) -> Vec<Signal> {
        let constant_bits = const_bits(constant, width);
        self.add_unsigned_bits(bits, &constant_bits, width)
            .into_iter()
            .take(width)
            .collect()
    }

    fn add_mod31_bits(&mut self, left: &[Signal], right: &[Signal]) -> Vec<Signal> {
        let sum = self.add_unsigned_bits(left, right, WIDTH);
        let low_all_ones = self.and_many(&sum[..WIDTH]);
        let reduce = self.or(Self::bit_at(&sum, WIDTH), low_all_ones);
        let subtract_modulus =
            self.add_const_wrapping_bits(&sum, (1u16 << (WIDTH + 1)) - FIELD_MODULUS, WIDTH + 1);
        self.mux_bits(reduce, &subtract_modulus[..WIDTH], &sum[..WIDTH])
    }

    fn sub_mod31_bits(&mut self, left: &[Signal], right: &[Signal]) -> Vec<Signal> {
        let right_complement = self.complement_field_bits(right);
        self.add_mod31_bits(left, &right_complement)
    }

    fn complement_field_bits(&mut self, bits: &[Signal]) -> Vec<Signal> {
        (0..WIDTH)
            .map(|bit| {
                let bit = Self::bit_at(bits, bit);
                self.not(bit)
            })
            .collect()
    }

    fn increment_by_signal_bits(&mut self, bits: &[Signal], increment: Signal) -> Vec<Signal> {
        let mut carry = increment;
        let mut out = Vec::with_capacity(WIDTH);
        for bit in 0..WIDTH {
            let input = Self::bit_at(bits, bit);
            let sum = self.xor(input.clone(), carry.clone());
            carry = self.and(input, carry);
            out.push(sum);
        }
        out
    }

    fn xor_carry_out_into(
        &mut self,
        left: &Signal,
        right: &Signal,
        carry_in: &Signal,
        target: QubitId,
    ) {
        self.xor_and_into(left, right, target);
        let left_xor_right = self.xor(left.clone(), right.clone());
        self.xor_and_into(carry_in, &left_xor_right, target);
    }

    fn xor_sum_bit_into(
        &mut self,
        left: &Signal,
        right: &Signal,
        carry_in: &Signal,
        target: QubitId,
    ) {
        self.xor_signal_into(left, target);
        self.xor_signal_into(right, target);
        self.xor_signal_into(carry_in, target);
    }

    fn xor_add_mod31_signals_into(
        &mut self,
        left: &[Signal],
        right: &[Signal],
        target: &[QubitId],
    ) {
        let sums = self.hold_qubits(WIDTH);
        let carries = self.hold_qubits(WIDTH);

        for bit in 0..WIDTH {
            let left_bit = Self::bit_at(left, bit);
            let right_bit = Self::bit_at(right, bit);
            let carry_in = if bit == 0 {
                Signal::constant(false)
            } else {
                Signal::qubit(carries[bit - 1])
            };
            self.xor_carry_out_into(&left_bit, &right_bit, &carry_in, carries[bit]);
            self.xor_sum_bit_into(&left_bit, &right_bit, &carry_in, sums[bit]);
        }

        let reduced_input =
            self.increment_by_signal_bits(&qubit_signals(&sums), Signal::qubit(carries[WIDTH - 1]));
        let all_ones = self.and_many(&reduced_input);
        let not_all_ones = self.not(all_ones);
        for (bit, &target) in target.iter().take(WIDTH).enumerate() {
            let reduced_bit = self.and(Self::bit_at(&reduced_input, bit), not_all_ones.clone());
            self.xor_signal_into(&reduced_bit, target);
        }

        for bit in (0..WIDTH).rev() {
            let left_bit = Self::bit_at(left, bit);
            let right_bit = Self::bit_at(right, bit);
            let carry_in = if bit == 0 {
                Signal::constant(false)
            } else {
                Signal::qubit(carries[bit - 1])
            };
            self.xor_sum_bit_into(&left_bit, &right_bit, &carry_in, sums[bit]);
            self.xor_carry_out_into(&left_bit, &right_bit, &carry_in, carries[bit]);
        }

        self.release_hold_qubits(carries);
        self.release_hold_qubits(sums);
    }

    fn rotate_mod31_bits(bits: &[Signal], shift: usize) -> Vec<Signal> {
        let shift = shift % WIDTH;
        (0..WIDTH)
            .map(|out_bit| Self::bit_at(bits, (out_bit + WIDTH - shift) % WIDTH))
            .collect()
    }

    fn mul_const_mod31_bits(&mut self, input: &[Signal], factor: u16) -> Vec<Signal> {
        let factor = factor % FIELD_MODULUS;
        let mut acc = const_bits(0, WIDTH);
        for shift in 0..WIDTH {
            if ((factor >> shift) & 1) != 0 {
                let shifted = Self::rotate_mod31_bits(input, shift);
                acc = self.add_mod31_bits(&acc, &shifted);
            }
        }
        acc
    }

    fn xor_controlled_rotated_field_into(
        &mut self,
        input: &[QubitId],
        selector: QubitId,
        shift: usize,
        target: &[QubitId],
    ) {
        let shift = shift % WIDTH;
        for (out_bit, &target) in target.iter().take(WIDTH).enumerate() {
            let source = input[(out_bit + WIDTH - shift) % WIDTH];
            self.xor_and_into(&Signal::qubit(selector), &Signal::qubit(source), target);
        }
    }

    fn xor_mul_mod31_into(&mut self, left: &[QubitId], right: &[QubitId], target: &[QubitId]) {
        let mut terms = Vec::with_capacity(WIDTH);
        let mut accs = Vec::with_capacity(WIDTH);
        let mut previous = const_bits(0, WIDTH);

        for (shift, &selector) in right.iter().take(WIDTH).enumerate() {
            let term = self.hold_qubits(WIDTH);
            self.xor_controlled_rotated_field_into(left, selector, shift, &term);

            let acc = self.hold_qubits(WIDTH);
            self.xor_add_mod31_signals_into(&previous, &qubit_signals(&term), &acc);

            previous = qubit_signals(&acc);
            terms.push(term);
            accs.push(acc);
        }

        if let Some(product) = accs.last() {
            self.xor_bits_into(product, target);
        }

        for index in (0..accs.len()).rev() {
            let previous = if index == 0 {
                const_bits(0, WIDTH)
            } else {
                qubit_signals(&accs[index - 1])
            };
            self.xor_add_mod31_signals_into(&previous, &qubit_signals(&terms[index]), &accs[index]);
            self.xor_controlled_rotated_field_into(left, right[index], index, &terms[index]);
            self.release_hold_qubits(accs[index].clone());
            self.release_hold_qubits(terms[index].clone());
        }
    }

    fn xor_inv_mod31_into(&mut self, input: &[QubitId], target: &[QubitId]) {
        let x2 = self.hold_qubits(WIDTH);
        let x4 = self.hold_qubits(WIDTH);
        let x8 = self.hold_qubits(WIDTH);
        let x16 = self.hold_qubits(WIDTH);
        let x24 = self.hold_qubits(WIDTH);
        let x28 = self.hold_qubits(WIDTH);

        self.xor_mul_mod31_into(input, input, &x2);
        self.xor_mul_mod31_into(&x2, &x2, &x4);
        self.xor_mul_mod31_into(&x4, &x4, &x8);
        self.xor_mul_mod31_into(&x8, &x8, &x16);
        self.xor_mul_mod31_into(&x16, &x8, &x24);
        self.xor_mul_mod31_into(&x24, &x4, &x28);
        self.xor_mul_mod31_into(&x28, input, target);

        self.xor_mul_mod31_into(&x24, &x4, &x28);
        self.xor_mul_mod31_into(&x16, &x8, &x24);
        self.xor_mul_mod31_into(&x8, &x8, &x16);
        self.xor_mul_mod31_into(&x4, &x4, &x8);
        self.xor_mul_mod31_into(&x2, &x2, &x4);
        self.xor_mul_mod31_into(input, input, &x2);

        self.release_hold_qubits(x28);
        self.release_hold_qubits(x24);
        self.release_hold_qubits(x16);
        self.release_hold_qubits(x8);
        self.release_hold_qubits(x4);
        self.release_hold_qubits(x2);
    }

    fn xor_and_into(&mut self, left: &Signal, right: &Signal, target: QubitId) {
        match (&left.0, &right.0) {
            (SignalKind::Const(false), _) | (_, SignalKind::Const(false)) => {}
            (SignalKind::Const(true), _) => {
                self.xor_signal_into(right, target);
            }
            (_, SignalKind::Const(true)) => {
                self.xor_signal_into(left, target);
            }
            (SignalKind::Qubit(a), SignalKind::Qubit(b)) if a == b => self.push_cx(*a, target),
            (SignalKind::Qubit(a), SignalKind::Qubit(b)) => self.push_ccx(*a, *b, target),
            (SignalKind::Qubit(a), _) => {
                let q = self.scratch();
                self.xor_signal_into(right, q);
                self.push_ccx(*a, q, target);
                self.xor_signal_into(right, q);
                self.release_scratch(q);
            }
            (_, SignalKind::Qubit(a)) => {
                let q = self.scratch();
                self.xor_signal_into(left, q);
                self.push_ccx(*a, q, target);
                self.xor_signal_into(left, q);
                self.release_scratch(q);
            }
            _ => {
                let left_q = self.scratch();
                self.xor_signal_into(left, left_q);
                let right_q = self.scratch();
                self.xor_signal_into(right, right_q);
                self.push_ccx(left_q, right_q, target);
                self.xor_signal_into(right, right_q);
                self.release_scratch(right_q);
                self.xor_signal_into(left, left_q);
                self.release_scratch(left_q);
            }
        }
    }

    fn xor_signal_into(&mut self, signal: &Signal, target: QubitId) {
        match &signal.0 {
            SignalKind::Const(false) => {}
            SignalKind::Const(true) => self.push_x(target),
            SignalKind::Qubit(control) => self.push_cx(*control, target),
            SignalKind::Expr(expr) => match expr.as_ref() {
                Expr::Not(input) => {
                    self.push_x(target);
                    self.xor_signal_into(input, target);
                }
                Expr::Xor(left, right) => {
                    self.xor_signal_into(left, target);
                    self.xor_signal_into(right, target);
                }
                Expr::And(left, right) => self.xor_and_into(left, right, target),
            },
        }
    }

    fn copy_bits_ops(&mut self, signals: &[Signal], targets: &[QubitId]) {
        for (signal, &target) in signals.iter().zip(targets) {
            self.xor_signal_into(signal, target);
        }
    }

    pub(crate) fn finish_segment(
        &mut self,
        outputs: Vec<(Vec<Signal>, Vec<QubitId>)>,
        flag_outputs: Vec<(Signal, QubitId)>,
    ) {
        for (signals, targets) in outputs {
            self.copy_bits_ops(&signals, &targets);
        }
        for (signal, target) in flag_outputs {
            self.xor_signal_into(&signal, target);
        }
    }

    pub(crate) fn finish_sink(self) -> S {
        debug_assert!(self.recording.is_empty());
        self.sink
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops_io::VecOpSink;
    use crate::sim::Simulator;
    use sha3::{digest::ExtendableOutput, Shake256};

    fn set_qubits(sim: &mut Simulator<'_, sha3::Shake256Reader>, bits: &[QubitId], value: u16) {
        for (bit, &qubit) in bits.iter().enumerate() {
            *sim.qubit_mut(qubit) = u64::from(((value >> bit) & 1) != 0);
        }
    }

    fn read_qubits(sim: &Simulator<'_, sha3::Shake256Reader>, bits: &[QubitId]) -> u16 {
        bits.iter().enumerate().fold(0u16, |acc, (bit, &qubit)| {
            acc | (((sim.qubit(qubit) & 1) as u16) << bit)
        })
    }

    fn assert_clean_ancilla(
        sim: &Simulator<'_, sha3::Shake256Reader>,
        public: &[QubitId],
        total_qubits: usize,
    ) {
        for qubit in 0..total_qubits {
            let qubit = QubitId(qubit as u64);
            if !public.contains(&qubit) {
                assert_eq!(sim.qubit(qubit), 0, "ancilla q{} was not cleaned", qubit.0);
            }
        }
    }

    fn total_qubits(ops: &[Op]) -> usize {
        ops.iter()
            .flat_map(|op| [op.q_target.0, op.q_control1.0, op.q_control2.0])
            .filter(|&qubit| qubit != u64::MAX)
            .max()
            .map(|qubit| qubit as usize + 1)
            .unwrap_or(0)
    }

    struct BinaryHarness {
        ops: Vec<Op>,
        left: Vec<QubitId>,
        right: Vec<QubitId>,
        target: Vec<QubitId>,
        total_qubits: usize,
    }

    impl BinaryHarness {
        fn new(operation: BinaryFieldOperation) -> Self {
            let mut builder = Builder::with_sink(VecOpSink::default());
            let left = builder.alloc_qubits(WIDTH);
            let right = builder.alloc_qubits(WIDTH);
            let target = builder.alloc_qubits(WIDTH);
            builder.xor_field_binary_into(operation, &left, &right, &target);
            let ops = builder.finish_sink().into_ops();
            let total_qubits = total_qubits(&ops);
            Self {
                ops,
                left,
                right,
                target,
                total_qubits,
            }
        }

        fn run(&self, left_value: u16, right_value: u16) -> u16 {
            let mut xof = Shake256::default().finalize_xof();
            let mut sim = Simulator::new(self.total_qubits, 0, &mut xof);
            set_qubits(&mut sim, &self.left, left_value);
            set_qubits(&mut sim, &self.right, right_value);
            sim.apply_iter_masked(self.ops.iter(), 1);
            assert_eq!(read_qubits(&sim, &self.left), left_value);
            assert_eq!(read_qubits(&sim, &self.right), right_value);
            assert_clean_ancilla(
                &sim,
                &[self.left.clone(), self.right.clone(), self.target.clone()].concat(),
                sim.num_qubits,
            );
            read_qubits(&sim, &self.target)
        }
    }

    struct UnaryHarness {
        ops: Vec<Op>,
        input: Vec<QubitId>,
        target: Vec<QubitId>,
        total_qubits: usize,
    }

    impl UnaryHarness {
        fn new(operation: UnaryFieldOperation) -> Self {
            let mut builder = Builder::with_sink(VecOpSink::default());
            let input = builder.alloc_qubits(WIDTH);
            let target = builder.alloc_qubits(WIDTH);
            builder.xor_field_unary_into(operation, &input, &target);
            let ops = builder.finish_sink().into_ops();
            let total_qubits = total_qubits(&ops);
            Self {
                ops,
                input,
                target,
                total_qubits,
            }
        }

        fn run(&self, input_value: u16) -> u16 {
            let mut xof = Shake256::default().finalize_xof();
            let mut sim = Simulator::new(self.total_qubits, 0, &mut xof);
            set_qubits(&mut sim, &self.input, input_value);
            sim.apply_iter_masked(self.ops.iter(), 1);
            assert_eq!(read_qubits(&sim, &self.input), input_value);
            assert_clean_ancilla(
                &sim,
                &[self.input.clone(), self.target.clone()].concat(),
                sim.num_qubits,
            );
            read_qubits(&sim, &self.target)
        }
    }

    fn inv_expected(value: u16) -> u16 {
        if value == 0 {
            return 0;
        }
        (1..FIELD_MODULUS)
            .find(|candidate| (value * candidate) % FIELD_MODULUS == 1)
            .unwrap()
    }

    #[test]
    fn field_binary_circuits_match_f31_arithmetic() {
        let add = BinaryHarness::new(BinaryFieldOperation::Add);
        let sub = BinaryHarness::new(BinaryFieldOperation::Sub);
        let mul = BinaryHarness::new(BinaryFieldOperation::Mul);
        for left in 0..FIELD_MODULUS {
            for right in 0..FIELD_MODULUS {
                assert_eq!(add.run(left, right), (left + right) % FIELD_MODULUS);
                assert_eq!(
                    sub.run(left, right),
                    (left + FIELD_MODULUS - right) % FIELD_MODULUS
                );
                assert_eq!(mul.run(left, right), (left * right) % FIELD_MODULUS);
            }
        }
    }

    #[test]
    fn field_unary_circuits_match_f31_arithmetic() {
        let mul3 = UnaryHarness::new(UnaryFieldOperation::MulConst(3));
        let inv = UnaryHarness::new(UnaryFieldOperation::Inv);
        for value in 0..FIELD_MODULUS {
            assert_eq!(mul3.run(value), (value * 3) % FIELD_MODULUS);
            assert_eq!(inv.run(value), inv_expected(value));
        }
    }
}
