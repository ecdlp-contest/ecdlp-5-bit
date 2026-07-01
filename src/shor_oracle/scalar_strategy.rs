//! Editable scalar-multiplication schedule for the 5-bit Shor oracle.
//!
//! This file may choose how to store, recompute, and clear arithmetic point
//! powers. It only receives opaque handles from `scalar_api`, so it cannot emit
//! raw gates or select from point-value tables.

use crate::shor_oracle::scalar_api::ScalarMulApi;

pub(crate) fn scalar_mul_into(ctx: &mut impl ScalarMulApi) {
    let base = ctx.base_point();

    let w0 = ctx.alloc_point(); // 2P, kept throughout
    let w1 = ctx.alloc_point(); // 4P, then reused for 16P
    let w2 = ctx.alloc_point(); // 8P

    ctx.double_xor(base, w0);
    ctx.double_xor(w0.as_point(), w1);
    ctx.double_xor(w1.as_point(), w2);

    ctx.add_selected(base, ctx.scalar_bit(0));
    ctx.add_selected(w0.as_point(), ctx.scalar_bit(1));
    ctx.add_selected(w1.as_point(), ctx.scalar_bit(2));
    ctx.add_selected(w2.as_point(), ctx.scalar_bit(3));

    // Reuse the 4P slot for 16P, then recreate 4P for cleanup of 8P.
    ctx.double_xor(w0.as_point(), w1); // w1 ← 0
    ctx.double_xor(w2.as_point(), w1); // w1 ← 16P
    ctx.add_selected(w1.as_point(), ctx.scalar_bit(4));

    ctx.double_xor(w2.as_point(), w1); // w1 ← 0
    ctx.double_xor(w0.as_point(), w1); // w1 ← 4P
    ctx.double_xor(w1.as_point(), w2); // w2 ← 0
    ctx.double_xor(w0.as_point(), w1); // w1 ← 0
    ctx.double_xor(base, w0); // w0 ← 0
}
