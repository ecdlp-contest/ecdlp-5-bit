/// This file contains code for simulating kickmix circuits.
use crate::circuit::{BitId, Op, OperationType, QubitId, NO_BIT, NO_QUBIT};
use ruint::aliases::U256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimStats {
    pub clifford_gates: u64,
    pub toffoli_gates: u64,
    pub ccx_gates: u64,
    pub ccz_gates: u64,
    pub toffoli_depth: u64,
}

pub struct Simulator<'a, R: sha3::digest::XofReader> {
    pub phase: u64,
    pub qubits: Vec<u64>,
    pub bits: Vec<u64>,
    pub num_qubits: usize,
    pub num_bits: usize,
    pub xof: &'a mut R,
    pub stats: SimStats,
}

fn op_qubits(op: &Op) -> Vec<usize> {
    let mut qubits = Vec::with_capacity(3);
    for qubit in [op.q_target, op.q_control1, op.q_control2] {
        if qubit != NO_QUBIT {
            let index = qubit.0 as usize;
            if !qubits.contains(&index) {
                qubits.push(index);
            }
        }
    }
    qubits
}

fn max_touched_qubit_layer(qubits: &[usize], qubit_layers: &[[u64; 64]], shot: usize) -> u64 {
    qubits
        .iter()
        .filter_map(|&q| qubit_layers.get(q))
        .map(|layers| layers[shot])
        .max()
        .unwrap_or(0)
}

fn update_quantum_dependency_layers(
    qubits: &[usize],
    cond: u64,
    cond_layers: [u64; 64],
    qubit_layers: &mut [[u64; 64]],
) {
    for shot in 0..64 {
        if ((cond >> shot) & 1) == 0 {
            continue;
        }
        let layer = cond_layers[shot].max(max_touched_qubit_layer(qubits, qubit_layers, shot));
        for &q in qubits {
            if let Some(layers) = qubit_layers.get_mut(q) {
                layers[shot] = layer;
            }
        }
    }
}

fn update_bit_dependency_layers(
    bit: BitId,
    cond: u64,
    cond_layers: [u64; 64],
    bit_layers: &mut [[u64; 64]],
) {
    if bit == NO_BIT {
        return;
    }
    if let Some(layers) = bit_layers.get_mut(bit.0 as usize) {
        for shot in 0..64 {
            if ((cond >> shot) & 1) != 0 {
                layers[shot] = cond_layers[shot];
            }
        }
    }
}

fn update_depth_layers(
    op: &Op,
    cond: u64,
    cond_layers: [u64; 64],
    qubit_layers: &mut [[u64; 64]],
    bit_layers: &mut [[u64; 64]],
    batch_toffoli_depth: &mut [u64; 64],
) {
    let qubits = op_qubits(op);
    match op.kind {
        OperationType::CCX | OperationType::CCZ => {
            for shot in 0..64 {
                if ((cond >> shot) & 1) == 0 {
                    continue;
                }
                let layer =
                    cond_layers[shot].max(max_touched_qubit_layer(&qubits, qubit_layers, shot)) + 1;
                for &q in &qubits {
                    if let Some(layers) = qubit_layers.get_mut(q) {
                        layers[shot] = layer;
                    }
                }
                batch_toffoli_depth[shot] = batch_toffoli_depth[shot].max(layer);
            }
        }
        OperationType::CX
        | OperationType::CZ
        | OperationType::Swap
        | OperationType::X
        | OperationType::Z
        | OperationType::R => {
            update_quantum_dependency_layers(&qubits, cond, cond_layers, qubit_layers);
        }
        OperationType::Hmr => {
            let mut measurement_layers = cond_layers;
            for shot in 0..64 {
                if ((cond >> shot) & 1) != 0 {
                    measurement_layers[shot] =
                        cond_layers[shot].max(max_touched_qubit_layer(&qubits, qubit_layers, shot));
                }
            }
            update_quantum_dependency_layers(&qubits, cond, measurement_layers, qubit_layers);
            update_bit_dependency_layers(op.c_target, cond, measurement_layers, bit_layers);
        }
        OperationType::BitInvert | OperationType::BitStore0 | OperationType::BitStore1 => {
            update_bit_dependency_layers(op.c_target, cond, cond_layers, bit_layers);
        }
        OperationType::AppendToRegister
        | OperationType::Register
        | OperationType::Neg
        | OperationType::PushCondition
        | OperationType::PopCondition
        | OperationType::DebugPrint => {}
    }
}

impl<'a, R: sha3::digest::XofReader> Simulator<'a, R> {
    pub fn new(num_qubits: usize, num_bits: usize, xof: &'a mut R) -> Self {
        let qubits = vec![0; num_qubits];
        let bits = vec![0; num_bits];

        Self {
            phase: 0,
            qubits,
            bits,
            num_qubits,
            num_bits,
            xof,
            stats: SimStats::default(),
        }
    }

    #[inline(always)]
    pub fn qubit(&self, id: QubitId) -> u64 {
        self.qubits[id.0 as usize]
    }

    #[inline(always)]
    pub fn qubit_mut(&mut self, id: QubitId) -> &mut u64 {
        &mut self.qubits[id.0 as usize]
    }

    #[inline(always)]
    pub fn bit(&self, id: BitId) -> u64 {
        self.bits[id.0 as usize]
    }

    #[inline(always)]
    pub fn bit_mut(&mut self, id: BitId) -> &mut u64 {
        &mut self.bits[id.0 as usize]
    }

    pub fn clear_for_shot(&mut self) {
        for e in &mut self.qubits {
            *e = 0;
        }
        for e in &mut self.bits {
            *e = 0;
        }
        self.phase = 0;
    }

    pub fn apply_iter<'b>(&mut self, ops: impl Iterator<Item = &'b Op>) {
        self.apply_iter_masked(ops, u64::MAX);
    }

    pub fn apply_iter_masked<'b>(&mut self, ops: impl Iterator<Item = &'b Op>, active_mask: u64) {
        let mut condition_stack = Vec::new();
        let mut current_base_condition = active_mask;
        let mut current_base_condition_layers = [0u64; 64];
        let mut qubit_layers = vec![[0u64; 64]; self.num_qubits];
        let mut bit_layers = vec![[0u64; 64]; self.num_bits];
        let mut batch_toffoli_depth = [0u64; 64];

        for op in ops {
            let mut cond = current_base_condition;
            let mut cond_layers = current_base_condition_layers;
            if op.c_condition != NO_BIT {
                cond &= self.bit(op.c_condition);
                let bit_layer = bit_layers[op.c_condition.0 as usize];
                for shot in 0..64 {
                    cond_layers[shot] = cond_layers[shot].max(bit_layer[shot]);
                }
            }

            let executed_shots = cond.count_ones() as u64;

            match op.kind {
                OperationType::CCX => {
                    self.stats.toffoli_gates += executed_shots;
                    self.stats.ccx_gates += executed_shots;
                }
                OperationType::CCZ => {
                    self.stats.toffoli_gates += executed_shots;
                    self.stats.ccz_gates += executed_shots;
                }
                OperationType::CX
                | OperationType::CZ
                | OperationType::Swap
                | OperationType::R
                | OperationType::Hmr => {
                    self.stats.clifford_gates += executed_shots;
                }
                // Note: X and Z are not considered Clifford gates in the
                // stats because they can be tracked in the classical control system.
                // They don't need to cause something to happen on the quantum computer.
                _ => {}
            }

            update_depth_layers(
                op,
                cond,
                cond_layers,
                &mut qubit_layers,
                &mut bit_layers,
                &mut batch_toffoli_depth,
            );

            match op.kind {
                OperationType::CCX => {
                    let v = cond & self.qubit(op.q_control1) & self.qubit(op.q_control2);
                    *self.qubit_mut(op.q_target) ^= v;
                }
                OperationType::CX => {
                    let v = cond & self.qubit(op.q_control1);
                    *self.qubit_mut(op.q_target) ^= v;
                }
                OperationType::Swap => {
                    let mut q_c1 = self.qubit(op.q_control1);
                    let mut q_t = self.qubit(op.q_target);
                    q_c1 ^= q_t;
                    q_t ^= cond & q_c1;
                    q_c1 ^= q_t;
                    *self.qubit_mut(op.q_control1) = q_c1;
                    *self.qubit_mut(op.q_target) = q_t;
                }
                OperationType::X => {
                    *self.qubit_mut(op.q_target) ^= cond;
                }
                OperationType::CCZ => {
                    let v = cond
                        & self.qubit(op.q_target)
                        & self.qubit(op.q_control1)
                        & self.qubit(op.q_control2);
                    self.phase ^= v;
                }
                OperationType::CZ => {
                    let v = cond & self.qubit(op.q_target) & self.qubit(op.q_control1);
                    self.phase ^= v;
                }
                OperationType::Z => {
                    let v = cond & self.qubit(op.q_target);
                    self.phase ^= v;
                }
                OperationType::Neg => {
                    self.phase ^= cond;
                }
                OperationType::Hmr => {
                    let mut buf = [0u8; 8];
                    self.xof.read(&mut buf);
                    let rng_val = u64::from_le_bytes(buf);
                    *self.bit_mut(op.c_target) &= !cond;
                    *self.bit_mut(op.c_target) ^= rng_val & cond;
                    self.phase ^= self.qubit(op.q_target) & rng_val & cond;
                    *self.qubit_mut(op.q_target) &= !cond;
                }
                OperationType::R => {
                    let mut buf = [0u8; 8];
                    self.xof.read(&mut buf);
                    let rng_val = u64::from_le_bytes(buf);
                    self.phase ^= self.qubit(op.q_target) & rng_val & cond;
                    *self.qubit_mut(op.q_target) &= !cond;
                }
                OperationType::BitInvert => {
                    *self.bit_mut(op.c_target) ^= cond;
                }
                OperationType::BitStore0 => {
                    *self.bit_mut(op.c_target) &= !cond;
                }
                OperationType::BitStore1 => {
                    *self.bit_mut(op.c_target) |= cond;
                }
                OperationType::AppendToRegister
                | OperationType::Register
                | OperationType::DebugPrint => {}
                OperationType::PushCondition => {
                    condition_stack.push((current_base_condition, current_base_condition_layers));
                    current_base_condition &= self.bit(op.c_condition);
                    current_base_condition_layers = cond_layers;
                }
                OperationType::PopCondition => {
                    if let Some(val) = condition_stack.pop() {
                        (current_base_condition, current_base_condition_layers) = val;
                    }
                }
            }
        }

        for shot in 0..64 {
            if ((active_mask >> shot) & 1) != 0 {
                self.stats.toffoli_depth += batch_toffoli_depth[shot];
            }
        }
    }

    /// Writes an integer into the qubits/bits of a register.
    ///
    /// Args:
    ///     reg: The qubits and bits making up the register, in little endian order.
    ///         CAUTION: Writes are unchecked!
    ///             Only pass in bits and qubits consistent with num_bits and num_qubits!
    ///         Caution: if a qubit or bit appears multiple times, the write to the more
    ///             significant bit position will overwrite prior writes.
    ///     val: The value to write into the bits/qubits.
    ///     shot_idx: The simulator tracks 64 shots in parallel. This is which shot to write to.
    pub fn set_register(&mut self, reg: &[crate::circuit::QubitOrBit], val: U256, shot_idx: usize) {
        for (i, item) in reg.iter().enumerate() {
            let bit_val = val.bit(i);
            match item {
                crate::circuit::QubitOrBit::Qubit(id) => {
                    if bit_val {
                        *self.qubit_mut(*id) |= 1 << shot_idx;
                    } else {
                        *self.qubit_mut(*id) &= !(1 << shot_idx);
                    }
                }
                crate::circuit::QubitOrBit::Bit(id) => {
                    if bit_val {
                        *self.bit_mut(*id) |= 1 << shot_idx;
                    } else {
                        *self.bit_mut(*id) &= !(1 << shot_idx);
                    }
                }
            }
        }
    }

    /// Reads the qubits/bits of a register as an integer.
    ///
    /// Args:
    ///     reg: The qubits and bits making up the register, in little endian order.
    ///         CAUTION: Reads are unchecked!
    ///             Only pass in bits and qubits consistent with num_bits and num_qubits!
    ///     shot_idx: The simulator tracks 64 shots in parallel. This is which shot to read from.
    ///
    /// Returns:
    ///     The requested integer.
    pub fn get_register(&self, reg: &[crate::circuit::QubitOrBit], shot_idx: usize) -> U256 {
        let mut v = U256::ZERO;
        for (i, item) in reg.iter().enumerate() {
            let bit_val = match item {
                crate::circuit::QubitOrBit::Qubit(id) => (self.qubit(*id) >> shot_idx) & 1,
                crate::circuit::QubitOrBit::Bit(id) => (self.bit(*id) >> shot_idx) & 1,
            };
            v.set_bit(i, bit_val != 0);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha3::digest::{ExtendableOutput, Update};

    fn xof() -> sha3::Shake256Reader {
        let mut hasher = sha3::Shake256::default();
        hasher.update(b"sim-depth-test");
        hasher.finalize_xof()
    }

    fn simulator(num_qubits: usize, num_bits: usize) -> Simulator<'static, sha3::Shake256Reader> {
        let reader = Box::leak(Box::new(xof()));
        Simulator::new(num_qubits, num_bits, reader)
    }

    fn three_qubit_op(kind: OperationType, q_control2: u64, q_control1: u64, q_target: u64) -> Op {
        let mut op = Op::empty();
        op.kind = kind;
        op.q_control2 = QubitId(q_control2);
        op.q_control1 = QubitId(q_control1);
        op.q_target = QubitId(q_target);
        op
    }

    fn cx_op(q_control1: u64, q_target: u64) -> Op {
        let mut op = Op::empty();
        op.kind = OperationType::CX;
        op.q_control1 = QubitId(q_control1);
        op.q_target = QubitId(q_target);
        op
    }

    fn bit_store(kind: OperationType, bit: u64) -> Op {
        let mut op = Op::empty();
        op.kind = kind;
        op.c_target = BitId(bit);
        op
    }

    fn condition_op(kind: OperationType, bit: u64) -> Op {
        let mut op = Op::empty();
        op.kind = kind;
        op.c_condition = BitId(bit);
        op
    }

    #[test]
    fn disjoint_toffolis_share_a_depth_layer() {
        let ops = vec![
            three_qubit_op(OperationType::CCX, 0, 1, 2),
            three_qubit_op(OperationType::CCZ, 3, 4, 5),
        ];
        let mut sim = simulator(6, 0);

        sim.apply_iter_masked(ops.iter(), 1);

        assert_eq!(sim.stats.toffoli_depth, 1);
    }

    #[test]
    fn shared_qubit_toffolis_advance_depth() {
        let ops = vec![
            three_qubit_op(OperationType::CCX, 0, 1, 2),
            three_qubit_op(OperationType::CCX, 2, 3, 4),
        ];
        let mut sim = simulator(5, 0);

        sim.apply_iter_masked(ops.iter(), 1);

        assert_eq!(sim.stats.toffoli_depth, 2);
    }

    #[test]
    fn clifford_dependency_links_nonclifford_layers() {
        let ops = vec![
            three_qubit_op(OperationType::CCX, 0, 1, 2),
            cx_op(2, 3),
            three_qubit_op(OperationType::CCX, 3, 4, 5),
        ];
        let mut sim = simulator(6, 0);

        sim.apply_iter_masked(ops.iter(), 1);

        assert_eq!(sim.stats.toffoli_depth, 2);
    }

    #[test]
    fn false_condition_does_not_contribute_executed_depth() {
        let ops = vec![
            bit_store(OperationType::BitStore1, 0),
            condition_op(OperationType::PushCondition, 0),
            three_qubit_op(OperationType::CCX, 0, 1, 2),
            condition_op(OperationType::PopCondition, 0),
            bit_store(OperationType::BitStore0, 0),
            condition_op(OperationType::PushCondition, 0),
            three_qubit_op(OperationType::CCX, 3, 4, 5),
            condition_op(OperationType::PopCondition, 0),
        ];
        let mut sim = simulator(6, 1);

        sim.apply_iter_masked(ops.iter(), 1);

        assert_eq!(sim.stats.toffoli_gates, 1);
        assert_eq!(sim.stats.toffoli_depth, 1);
    }
}
