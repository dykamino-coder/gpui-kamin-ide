//! The kept conversions of `StyleConversions` and the value-context part of their key.

use crate::layout::block::sizing::BoxDimensions;
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;
use gpui::StyleRefinement;
use std::rc::Rc;

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

/// One kept conversion: its input, under `Values`, and the element style it gave.
pub(super) struct Conversion {
    /// `BoxStyle::group_key` of `input`: the first probe.
    pub(super) key: u64,
    pub(super) input: BoxStyle,
    pub(super) dimensions: BoxDimensions,
    pub(super) values: Values,
    pub(super) style: Rc<StyleRefinement>,
}
