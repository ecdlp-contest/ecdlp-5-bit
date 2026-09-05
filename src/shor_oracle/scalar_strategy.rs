//! Editable scalar-multiplication schedule for the 5-bit Shor oracle.
//!
//! This file may choose how to store, recompute, and clear arithmetic point
//! powers. It only receives opaque handles from `scalar_api`, so it cannot emit
//! raw gates or select from point-value tables.

use crate::shor_oracle::scalar_api::ScalarMulApi;

pub(crate) fn scalar_mul_into(ctx: &mut impl ScalarMulApi) {
    let base = ctx.base_point();

    let w0 = ctx.alloc_point(); // 2P, then 16P, then 2P again
    let w1 = ctx.alloc_point(); // 4P
    let w2 = ctx.alloc_point(); // 8P

    ctx.add_selected(base, ctx.scalar_bit(0));

    ctx.double_xor(base, w0); // w0 ← 2P
    ctx.add_selected(w0.as_point(), ctx.scalar_bit(1));

    ctx.double_xor(w0.as_point(), w1); // w1 ← 4P
    ctx.double_xor(base, w0); // w0 ← 0

    // Run the doubling chain to 16P before the 4P/8P adds so that the
    // late-controlled-add tail shortens the per-shot Toffoli critical path.
    ctx.double_xor(w1.as_point(), w2); // w2 ← 8P
    ctx.double_xor(w2.as_point(), w0); // w0 ← 16P
    ctx.add_selected(w0.as_point(), ctx.scalar_bit(4));

    // Every intermediate stays a valid point encoding; cleanup cascades the
    // doubling chain downward, reusing w0 as a 2P erasure pebble.
    ctx.double_xor(w2.as_point(), w0); // w0 ← 0
    ctx.add_selected(w1.as_point(), ctx.scalar_bit(2));

    ctx.double_xor(base, w0); // w0 ← 2P
    ctx.add_selected(w2.as_point(), ctx.scalar_bit(3));

    ctx.double_xor(w1.as_point(), w2); // w2 ← 0
    ctx.double_xor(w0.as_point(), w1); // w1 ← 0
    ctx.double_xor(base, w0); // w0 ← 0
}
