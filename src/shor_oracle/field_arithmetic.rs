//! Contestant-editable field-kernel choices for the 5-bit Shor oracle.
//!
//! This module intentionally does not own the oracle builder, register layout,
//! segment boundaries, or primitive op sink. The trusted builder passes only
//! opaque per-field operands and targets into these functions, so implementations
//! cannot observe public point registers or write directly to the oracle output.

use super::builder::{
    BinaryFieldOperation, FieldEmitter, FieldInput, FieldOutput, UnaryFieldOperation,
    FIELD_MODULUS, WIDTH,
};
use crate::ops_io::OpSink;

fn xor_rotated_field_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    input: FieldInput<'_>,
    shift: usize,
    target: FieldOutput<'_>,
) {
    let input_bits = emitter.input_bits(input);
    let shift = shift % WIDTH;
    for out_bit in 0..WIDTH {
        if let Some(target_bit) = emitter.target_bit(target, out_bit) {
            let in_bit = (out_bit + WIDTH - shift) % WIDTH;
            emitter.xor_signal_into(&input_bits[in_bit], target_bit);
        }
    }
}

pub(crate) fn xor_add_mod_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    left: FieldInput<'_>,
    right: FieldInput<'_>,
    target: FieldOutput<'_>,
) {
    emitter.xor_binary_mod_into(BinaryFieldOperation::Add, left, right, target);
}

pub(crate) fn xor_sub_mod_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    left: FieldInput<'_>,
    right: FieldInput<'_>,
    target: FieldOutput<'_>,
) {
    emitter.xor_binary_mod_into(BinaryFieldOperation::Sub, left, right, target);
}

pub(crate) fn xor_mul_mod_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    left: FieldInput<'_>,
    right: FieldInput<'_>,
    target: FieldOutput<'_>,
) {
    emitter.xor_binary_mod_into(BinaryFieldOperation::Mul, left, right, target);
}

pub(crate) fn xor_mul_const_mod_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    input: FieldInput<'_>,
    factor: u16,
    target: FieldOutput<'_>,
) {
    match factor % FIELD_MODULUS {
        0 => {}
        1 => xor_rotated_field_into(emitter, input, 0, target),
        2 => xor_rotated_field_into(emitter, input, 1, target),
        4 => xor_rotated_field_into(emitter, input, 2, target),
        8 => xor_rotated_field_into(emitter, input, 3, target),
        16 => xor_rotated_field_into(emitter, input, 4, target),
        _ => emitter.xor_unary_mod_into(UnaryFieldOperation::MulConst(factor), input, target),
    }
}

pub(crate) fn xor_inv_mod_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    input: FieldInput<'_>,
    target: FieldOutput<'_>,
) {
    emitter.xor_unary_mod_into(UnaryFieldOperation::Inv, input, target);
}
