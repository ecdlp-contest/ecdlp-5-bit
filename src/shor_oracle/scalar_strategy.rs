//! Editable scalar-multiplication schedule for the 5-bit Shor oracle.
//!
//! This file may choose how to store, recompute, and clear arithmetic point
//! powers. It only receives opaque handles from `scalar_api`, so it cannot emit
//! raw gates or select from point-value tables.

use crate::shor_oracle::scalar_api::ScalarMulApi;

pub(crate) fn scalar_mul_into(ctx: &mut impl ScalarMulApi) {
    let base = ctx.base_point();

    let w0 = ctx.alloc_point(); // 2P, then 8P
    let w1 = ctx.alloc_point(); // 4P

    ctx.double_xor(base, w0);
    ctx.double_xor(w0.as_point(), w1);

    ctx.double_xor(base, w0); // w0 ← 0
    ctx.double_xor(w1.as_point(), w0); // w0 ← 8P

    let w2 = ctx.alloc_point(); // 16P, then a cleanup pebble
    ctx.double_xor(w0.as_point(), w2); // w2 ← 16P
    ctx.add_selected(w2.as_point(), ctx.scalar_bit(4));

    // Park 16P xor 2P in w2, then remove 16P so w2 becomes a reusable 2P
    // cleanup pebble. The invalid intermediate is never used as a source.
    ctx.double_xor(base, w2); // w2 ← 16P xor 2P
    ctx.double_xor(w0.as_point(), w2); // w2 ← 2P
    ctx.add_selected(w0.as_point(), ctx.scalar_bit(3));
    ctx.double_xor(w1.as_point(), w0); // w0 ← 0
    ctx.add_selected(w1.as_point(), ctx.scalar_bit(2));
    ctx.double_xor(w2.as_point(), w1); // w1 ← 0
    ctx.add_selected(w2.as_point(), ctx.scalar_bit(1));
    ctx.double_xor(base, w2); // w2 ← 0
    ctx.add_selected(base, ctx.scalar_bit(0));
}
