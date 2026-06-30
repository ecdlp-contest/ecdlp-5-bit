use crate::circuit::{Op, OperationType, QubitId, RegisterId};
use crate::ops_io::OpSink;
use std::rc::Rc;

pub(crate) const WIDTH: usize = 5;
pub(crate) const FIELD_MODULUS: u16 = 31;

pub(crate) fn bit_at(bits: &[Signal], index: usize) -> Signal {
    bits.get(index).cloned().unwrap_or(Signal::Const(false))
}

pub(crate) fn const_bits(value: u16, width: usize) -> Vec<Signal> {
    (0..width)
        .map(|bit| Signal::Const(((value >> bit) & 1) != 0))
        .collect()
}

fn inv_mod_const(value: u16, modulus: u16) -> u16 {
    let value = value % modulus;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Signal {
    Const(bool),
    Qubit(QubitId),
    Expr(Rc<Expr>),
    FieldBit(Rc<FieldExpr>, usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Expr {
    Not(Signal),
    Xor(Signal, Signal),
    And(Signal, Signal),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Add,
    Sub,
    Mul,
    MulConst(u16),
    Inv,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldExpr {
    Unary {
        kind: FieldKind,
        input: Vec<Signal>,
        modulus: u16,
    },
    Binary {
        kind: FieldKind,
        left: Vec<Signal>,
        right: Vec<Signal>,
        modulus: u16,
    },
}

fn field_inputs(expr: &FieldExpr) -> Vec<Signal> {
    match expr {
        FieldExpr::Unary { input, .. } => input.clone(),
        FieldExpr::Binary { left, right, .. } => {
            let mut inputs = left.clone();
            inputs.extend(right.iter().cloned());
            inputs
        }
    }
}

fn field_value(expr: &FieldExpr, assignment: usize) -> u16 {
    match expr {
        FieldExpr::Unary {
            kind,
            input: _,
            modulus,
        } => {
            let value = (assignment & ((1usize << WIDTH) - 1)) as u16;
            field_unary_value(kind, value, *modulus)
        }
        FieldExpr::Binary {
            kind,
            left: _,
            right: _,
            modulus,
        } => {
            let left = (assignment & ((1usize << WIDTH) - 1)) as u16;
            let right = ((assignment >> WIDTH) & ((1usize << WIDTH) - 1)) as u16;
            field_binary_value(kind, left, right, *modulus)
        }
    }
}

fn field_unary_value(kind: &FieldKind, value: u16, modulus: u16) -> u16 {
    match kind {
        FieldKind::MulConst(factor) => ((value % modulus) * (*factor % modulus)) % modulus,
        FieldKind::Inv => inv_mod_const(value, modulus),
        FieldKind::Add | FieldKind::Sub | FieldKind::Mul => {
            unreachable!("binary field kind used as unary")
        }
    }
}

fn field_binary_value(kind: &FieldKind, left: u16, right: u16, modulus: u16) -> u16 {
    let left = left % modulus;
    let right = right % modulus;
    match kind {
        FieldKind::Add => (left + right) % modulus,
        FieldKind::Sub => (left + modulus - right) % modulus,
        FieldKind::Mul => (left * right) % modulus,
        FieldKind::MulConst(_) | FieldKind::Inv => unreachable!("unary field kind used as binary"),
    }
}

pub(crate) struct Builder<S: OpSink> {
    sink: S,
    recording: Option<Vec<Op>>,
    free_scratch: Vec<QubitId>,
    field_cache: Vec<CachedField>,
    next_qubit: u64,
    next_register: u64,
}

struct CachedField {
    expr: Rc<FieldExpr>,
    bit: usize,
    qubit: QubitId,
}

impl<S: OpSink> Builder<S> {
    pub(crate) fn with_sink(sink: S) -> Self {
        Self {
            sink,
            recording: None,
            free_scratch: Vec::new(),
            field_cache: Vec::new(),
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
        if let Some(recording) = &mut self.recording {
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

    pub(crate) fn record<F>(&mut self, f: F) -> Vec<Op>
    where
        F: FnOnce(&mut Self),
    {
        assert!(
            self.recording.is_none(),
            "nested op recording is unsupported"
        );
        self.recording = Some(Vec::new());
        f(self);
        self.recording.take().unwrap()
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
            Signal::Const(value) => Signal::Const(!value),
            Signal::Expr(expr) => {
                if let Expr::Not(inner) = expr.as_ref() {
                    inner.clone()
                } else {
                    Signal::Expr(Rc::new(Expr::Not(Signal::Expr(expr))))
                }
            }
            value => Signal::Expr(Rc::new(Expr::Not(value))),
        }
    }

    fn same_signal(left: &Signal, right: &Signal) -> bool {
        match (left, right) {
            (Signal::Const(a), Signal::Const(b)) => a == b,
            (Signal::Qubit(a), Signal::Qubit(b)) => a == b,
            (Signal::Expr(a), Signal::Expr(b)) => Rc::ptr_eq(a, b),
            (Signal::FieldBit(a, a_bit), Signal::FieldBit(b, b_bit)) => {
                Rc::ptr_eq(a, b) && a_bit == b_bit
            }
            _ => false,
        }
    }

    fn xor(&mut self, left: Signal, right: Signal) -> Signal {
        match (left, right) {
            (Signal::Const(a), Signal::Const(b)) => Signal::Const(a ^ b),
            (Signal::Const(false), value) | (value, Signal::Const(false)) => value,
            (Signal::Const(true), value) | (value, Signal::Const(true)) => self.not(value),
            (Signal::Qubit(a), Signal::Qubit(b)) if a == b => Signal::Const(false),
            (a, b) if Self::same_signal(&a, &b) => Signal::Const(false),
            (a, b) => Signal::Expr(Rc::new(Expr::Xor(a, b))),
        }
    }

    pub(crate) fn and(&mut self, left: Signal, right: Signal) -> Signal {
        match (left, right) {
            (Signal::Const(a), Signal::Const(b)) => Signal::Const(a & b),
            (Signal::Const(false), _) | (_, Signal::Const(false)) => Signal::Const(false),
            (Signal::Const(true), value) | (value, Signal::Const(true)) => value,
            (Signal::Qubit(a), Signal::Qubit(b)) if a == b => Signal::Qubit(a),
            (a, b) if Self::same_signal(&a, &b) => a,
            (a, b) => Signal::Expr(Rc::new(Expr::And(a, b))),
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
        let mut acc = Signal::Const(true);
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
            Signal::Const(true) => when_true,
            Signal::Const(false) => when_false,
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

    pub(crate) fn add_mod(
        &mut self,
        left: &[Signal],
        right: &[Signal],
        modulus: u16,
    ) -> Vec<Signal> {
        self.field_binary_bits(FieldKind::Add, left, right, modulus)
    }

    pub(crate) fn sub_mod(
        &mut self,
        left: &[Signal],
        right: &[Signal],
        modulus: u16,
    ) -> Vec<Signal> {
        self.field_binary_bits(FieldKind::Sub, left, right, modulus)
    }

    pub(crate) fn mul_const_mod(
        &mut self,
        input: &[Signal],
        factor: u16,
        modulus: u16,
    ) -> Vec<Signal> {
        self.field_unary_bits(FieldKind::MulConst(factor), input, modulus)
    }

    pub(crate) fn mul_mod(
        &mut self,
        left: &[Signal],
        right: &[Signal],
        modulus: u16,
    ) -> Vec<Signal> {
        self.field_binary_bits(FieldKind::Mul, left, right, modulus)
    }

    pub(crate) fn inv_mod(&mut self, input: &[Signal], modulus: u16) -> Vec<Signal> {
        self.field_unary_bits(FieldKind::Inv, input, modulus)
    }

    fn field_unary_bits(&mut self, kind: FieldKind, input: &[Signal], modulus: u16) -> Vec<Signal> {
        let expr = Rc::new(FieldExpr::Unary {
            kind,
            input: (0..WIDTH).map(|bit| Self::bit_at(input, bit)).collect(),
            modulus,
        });
        (0..WIDTH)
            .map(|bit| Signal::FieldBit(expr.clone(), bit))
            .collect()
    }

    fn field_binary_bits(
        &mut self,
        kind: FieldKind,
        left: &[Signal],
        right: &[Signal],
        modulus: u16,
    ) -> Vec<Signal> {
        let expr = Rc::new(FieldExpr::Binary {
            kind,
            left: (0..WIDTH).map(|bit| Self::bit_at(left, bit)).collect(),
            right: (0..WIDTH).map(|bit| Self::bit_at(right, bit)).collect(),
            modulus,
        });
        (0..WIDTH)
            .map(|bit| Signal::FieldBit(expr.clone(), bit))
            .collect()
    }

    fn cached_field_qubit(&self, expr: &Rc<FieldExpr>, out_bit: usize) -> Option<QubitId> {
        self.field_cache
            .iter()
            .find(|cached| Rc::ptr_eq(&cached.expr, expr) && cached.bit == out_bit)
            .map(|cached| cached.qubit)
    }

    fn xor_field_bit_cached_into(&mut self, expr: &Rc<FieldExpr>, out_bit: usize, target: QubitId) {
        if let Some(qubit) = self.cached_field_qubit(expr, out_bit) {
            self.push_cx(qubit, target);
            return;
        }

        let qubit = self.scratch();
        self.xor_field_bit_uncached_into(expr.as_ref(), out_bit, qubit);
        self.field_cache.push(CachedField {
            expr: expr.clone(),
            bit: out_bit,
            qubit,
        });
        self.push_cx(qubit, target);
    }

    fn xor_field_bit_uncached_into(&mut self, expr: &FieldExpr, out_bit: usize, target: QubitId) {
        let inputs = field_inputs(expr);
        let n = inputs.len();
        let size = 1usize << n;
        let mut coeffs: Vec<bool> = (0..size)
            .map(|assignment| ((field_value(expr, assignment) >> out_bit) & 1) != 0)
            .collect();
        for bit in 0..n {
            for mask in 0..size {
                if (mask & (1usize << bit)) != 0 {
                    coeffs[mask] ^= coeffs[mask ^ (1usize << bit)];
                }
            }
        }

        for (mask, coeff) in coeffs.into_iter().enumerate() {
            if coeff {
                self.xor_monomial_signals_into(&inputs, mask, target);
            }
        }
    }

    fn xor_monomial_signals_into(&mut self, inputs: &[Signal], mask: usize, target: QubitId) {
        let mut controls = Vec::new();
        let mut computed: Vec<(Signal, QubitId)> = Vec::new();
        for (bit, input) in inputs.iter().enumerate() {
            if (mask & (1usize << bit)) == 0 {
                continue;
            }
            match input {
                Signal::Const(false) => {
                    for (signal, qubit) in computed.into_iter().rev() {
                        self.xor_signal_into(&signal, qubit);
                        self.release_scratch(qubit);
                    }
                    return;
                }
                Signal::Const(true) => {}
                Signal::Qubit(qubit) => {
                    if !controls.contains(qubit) {
                        controls.push(*qubit);
                    }
                }
                _ => {
                    let qubit = self.scratch();
                    self.xor_signal_into(input, qubit);
                    if !controls.contains(&qubit) {
                        controls.push(qubit);
                    }
                    computed.push((input.clone(), qubit));
                }
            }
        }

        match controls.len() {
            0 => self.push_x(target),
            1 => self.push_cx(controls[0], target),
            2 => self.push_ccx(controls[0], controls[1], target),
            _ => {
                let mut chain = Vec::with_capacity(controls.len() - 2);
                let first = self.scratch();
                self.push_ccx(controls[0], controls[1], first);
                chain.push(first);
                for &control in &controls[2..controls.len() - 1] {
                    let next = self.scratch();
                    self.push_ccx(*chain.last().unwrap(), control, next);
                    chain.push(next);
                }
                self.push_ccx(*chain.last().unwrap(), *controls.last().unwrap(), target);
                for (i, &control) in controls[2..controls.len() - 1].iter().enumerate().rev() {
                    self.push_ccx(chain[i], control, chain[i + 1]);
                }
                self.push_ccx(controls[0], controls[1], first);
                for qubit in chain.into_iter().rev() {
                    self.release_scratch(qubit);
                }
            }
        }

        for (signal, qubit) in computed.into_iter().rev() {
            self.xor_signal_into(&signal, qubit);
            self.release_scratch(qubit);
        }
    }

    fn xor_and_into(&mut self, left: &Signal, right: &Signal, target: QubitId) {
        match (left, right) {
            (Signal::Const(false), _) | (_, Signal::Const(false)) => {}
            (Signal::Const(true), value) | (value, Signal::Const(true)) => {
                self.xor_signal_into(value, target);
            }
            (Signal::Qubit(a), Signal::Qubit(b)) if a == b => self.push_cx(*a, target),
            (Signal::Qubit(a), Signal::Qubit(b)) => self.push_ccx(*a, *b, target),
            (Signal::Qubit(a), value) | (value, Signal::Qubit(a)) => {
                let q = self.scratch();
                self.xor_signal_into(value, q);
                self.push_ccx(*a, q, target);
                self.xor_signal_into(value, q);
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
        match signal {
            Signal::Const(false) => {}
            Signal::Const(true) => self.push_x(target),
            Signal::Qubit(control) => self.push_cx(*control, target),
            Signal::FieldBit(expr, out_bit) => {
                self.xor_field_bit_cached_into(expr, *out_bit, target)
            }
            Signal::Expr(expr) => match expr.as_ref() {
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
        debug_assert!(self.field_cache.is_empty());
        for (signals, targets) in outputs {
            self.copy_bits_ops(&signals, &targets);
        }
        for (signal, target) in flag_outputs {
            self.xor_signal_into(&signal, target);
        }
        while let Some(cached) = self.field_cache.pop() {
            self.xor_field_bit_uncached_into(cached.expr.as_ref(), cached.bit, cached.qubit);
            self.release_scratch(cached.qubit);
        }
    }

    pub(crate) fn finish_sink(self) -> S {
        debug_assert!(self.field_cache.is_empty());
        debug_assert!(self.recording.is_none());
        self.sink
    }
}
