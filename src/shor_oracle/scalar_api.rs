//! Trusted high-level scalar-multiplication scheduling API.
//!
//! Contestants may edit `scalar_strategy.rs`, but this module keeps raw qubits,
//! point registers, and primitive op emission behind opaque handles.

use super::{
    controlled_add_assign, hold_point, point_double_xor, release_point, Builder, PointRegister,
};
use crate::circuit::QubitId;
use crate::ops_io::OpSink;

pub(crate) const WIDTH: usize = super::builder::WIDTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PointHandle(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScratchPoint(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScalarBit(usize);

impl ScratchPoint {
    pub(crate) fn as_point(self) -> PointHandle {
        PointHandle(self.0)
    }
}

pub(crate) trait ScalarMulApi {
    fn base_point(&self) -> PointHandle;
    fn scalar_bit(&self, bit: usize) -> ScalarBit;
    fn alloc_point(&mut self) -> ScratchPoint;
    fn double_xor(&mut self, point: PointHandle, out: ScratchPoint);
    fn add_selected(&mut self, addend: PointHandle, selector: ScalarBit);
}

pub(crate) struct ScalarMulContext<'a, S: OpSink> {
    builder: &'a mut Builder<S>,
    scalar: &'a [QubitId],
    point: &'a PointRegister,
    out: &'a PointRegister,
    held: Vec<PointRegister>,
}

impl<'a, S: OpSink> ScalarMulContext<'a, S> {
    pub(crate) fn new(
        builder: &'a mut Builder<S>,
        scalar: &'a [QubitId],
        point: &'a PointRegister,
        out: &'a PointRegister,
    ) -> Self {
        Self {
            builder,
            scalar,
            point,
            out,
            held: Vec::new(),
        }
    }

    pub(crate) fn release_points(mut self) {
        for point in self.held.drain(..) {
            release_point(self.builder, point);
        }
    }

    fn resolve_point(&self, handle: PointHandle) -> PointRegister {
        if handle.0 == 0 {
            return self.point.clone();
        }
        self.held
            .get(handle.0 - 1)
            .cloned()
            .expect("invalid scalar-strategy point handle")
    }

    fn resolve_scratch(&self, handle: ScratchPoint) -> PointRegister {
        assert!(handle.0 != 0, "base point cannot be a scratch output");
        self.held
            .get(handle.0 - 1)
            .cloned()
            .expect("invalid scalar-strategy scratch handle")
    }
}

impl<S: OpSink> ScalarMulApi for ScalarMulContext<'_, S> {
    fn base_point(&self) -> PointHandle {
        PointHandle(0)
    }

    fn scalar_bit(&self, bit: usize) -> ScalarBit {
        assert!(bit < self.scalar.len(), "scalar bit index out of range");
        ScalarBit(bit)
    }

    fn alloc_point(&mut self) -> ScratchPoint {
        let handle = ScratchPoint(self.held.len() + 1);
        self.held.push(hold_point(self.builder));
        handle
    }

    fn double_xor(&mut self, point: PointHandle, out: ScratchPoint) {
        let point = self.resolve_point(point);
        let out = self.resolve_scratch(out);
        point_double_xor(self.builder, &point, &out);
    }

    fn add_selected(&mut self, addend: PointHandle, selector: ScalarBit) {
        let addend = self.resolve_point(addend);
        let out = self.out.clone();
        let selector = *self
            .scalar
            .get(selector.0)
            .expect("invalid scalar-strategy selector handle");
        controlled_add_assign(self.builder, &out, &addend, selector);
    }
}
