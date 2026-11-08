//! Value-context key of the conversions of `StyleConversions`.

use crate::values::ValueCx;

/// The parts of the value context a conversion reads.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Values {
    doc: *const crate::values::DocumentValues,
    frame: bool,
    viewport: (f32, f32),
    scheme: crate::style::values::value::Scheme,
    clearance_possible: bool,
}

impl Values {
    pub(super) fn of(vcx: ValueCx<'_>) -> Self {
        Self {
            doc: vcx.doc,
            frame: vcx.frame,
            viewport: vcx.viewport,
            scheme: vcx.scheme,
            clearance_possible: vcx.clearance_possible,
        }
    }
}
