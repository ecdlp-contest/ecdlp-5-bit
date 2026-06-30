//! Editable scalar-multiplication schedule for the 5-bit Shor oracle.
//!
//! This file may choose how to store, recompute, and clear arithmetic point
//! powers. It only receives opaque handles from `scalar_api`, so it cannot emit
//! raw gates or select from point-value tables.

use crate::shor_oracle::scalar_api::{ScalarMulApi, WIDTH};

pub(crate) fn scalar_mul_into(ctx: &mut impl ScalarMulApi) {
    let base = ctx.base_point();
    let powers: Vec<_> = (1..WIDTH).map(|_| ctx.alloc_point()).collect();

    if let Some(first_power) = powers.first() {
        ctx.double_xor(base, *first_power);
        for bit in 2..WIDTH {
            ctx.double_xor(powers[bit - 2].as_point(), powers[bit - 1]);
        }
    }

    for bit in 0..WIDTH {
        let addend = if bit == 0 {
            base
        } else {
            powers[bit - 1].as_point()
        };
        ctx.add_selected(addend, ctx.scalar_bit(bit));
    }

    for bit in (1..WIDTH).rev() {
        if bit == 1 {
            ctx.double_xor(base, powers[0]);
        } else {
            ctx.double_xor(powers[bit - 2].as_point(), powers[bit - 1]);
        }
    }
}
