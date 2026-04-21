//! Internal pipeline helpers shared by decoders.

use crate::model::Primitive;

/// Minimal sink abstraction used by decoders to emit primitives incrementally.
///
/// The public API still exposes a batch `Vec<Primitive>`, but decoders should
/// prefer emitting into a sink rather than depending on a concrete buffer.
pub(crate) trait PrimitiveSink {
    fn emit(&mut self, primitive: Primitive);
}

impl PrimitiveSink for Vec<Primitive> {
    fn emit(&mut self, primitive: Primitive) {
        self.push(primitive);
    }
}
