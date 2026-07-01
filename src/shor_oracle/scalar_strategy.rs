//! Editable scalar-multiplication schedule for the 5-bit Shor oracle.
//!
//! This file may choose how to store, recompute, and clear arithmetic point
//! powers. It only receives opaque handles from `scalar_api`, so it cannot emit
//! raw gates or select from point-value tables.

use crate::shor_oracle::scalar_api::{ScalarMulApi, WIDTH};

pub(crate) fn scalar_mul_into(ctx: &mut impl ScalarMulApi) {
    let base = ctx.base_point();

    // Allocate 3 scratch points instead of 4.
    // w0 holds 2P at first, then gets uncomputed and reused for 16P.
    // w1 holds 4P, w2 holds 8P throughout.
    let w0 = ctx.alloc_point();
    let w1 = ctx.alloc_point();
    let w2 = ctx.alloc_point();

    // Compute power chain 2P → 4P → 8P.
    ctx.double_xor(base, w0);
    ctx.double_xor(w0.as_point(), w1);
    ctx.double_xor(w1.as_point(), w2);

    // Accumulate bits 0–3.
    ctx.add_selected(base, ctx.scalar_bit(0));
    ctx.add_selected(w0.as_point(), ctx.scalar_bit(1));
    ctx.add_selected(w1.as_point(), ctx.scalar_bit(2));
    ctx.add_selected(w2.as_point(), ctx.scalar_bit(3));

    // Reuse w0 for 16P: uncompute 2P then load 16P = 2·(w2=8P).
    ctx.double_xor(base, w0);          // w0 ← 0
    ctx.double_xor(w2.as_point(), w0); // w0 ← 16P

    // Accumulate bit 4.
    ctx.add_selected(w0.as_point(), ctx.scalar_bit(4));

    // Uncompute all scratch back to zero.
    ctx.double_xor(w2.as_point(), w0); // w0 ← 0 (uncompute 16P)
    ctx.double_xor(w1.as_point(), w2); // w2 ← 0 (uncompute 8P)
    ctx.double_xor(base, w0);          // w0 ← 2P  (recompute to uncompute w1)
    ctx.double_xor(w0.as_point(), w1); // w1 ← 0 (uncompute 4P)
    ctx.double_xor(base, w0);          // w0 ← 0 (uncompute 2P)
}
