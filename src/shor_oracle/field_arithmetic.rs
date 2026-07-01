//! Contestant-editable field-kernel choices for the 5-bit Shor oracle.
//!
//! This module intentionally does not own the oracle builder, register layout,
//! segment boundaries, or primitive op sink. The trusted builder passes only
//! opaque per-field operands and targets into these functions, so implementations
//! cannot observe public point registers or write directly to the oracle output.

use super::builder::{
    BinaryFieldOperation, FieldEmitter, FieldInput, FieldOutput, UnaryFieldOperation,
};
use crate::ops_io::OpSink;

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
    emitter.xor_unary_mod_into(UnaryFieldOperation::MulConst(factor), input, target);
}

pub(crate) fn xor_inv_mod_into<S: OpSink>(
    emitter: &mut FieldEmitter<'_, S>,
    input: FieldInput<'_>,
    target: FieldOutput<'_>,
) {
    emitter.xor_unary_mod_into(UnaryFieldOperation::Inv, input, target);
}
