//! Входы записи стадии коробки: стадии своего стиля, документ, окно; причина промаха.

use crate::style::values::value::Scheme;
use crate::values::ValueCx;
use std::any::Any;
use std::rc::{Rc, Weak};

/// The allocation of one stage of a style; a weak handle keeps the address from reuse
/// without keeping the value alive, and an edit in place (`Rc::make_mut`) moves it.
pub(crate) type Identity = Option<Weak<dyn Any>>;

pub(crate) fn identity<T: 'static>(rc: &Rc<T>) -> Weak<dyn Any> {
    let weak: Weak<T> = Rc::downgrade(rc);
    weak
}

/// The stages of an element's style a box stage reads (cascaded, materialized, used,
/// layout facts, decoration input) and the query container size its slot holds.
pub struct OwnKey {
    stages: [Identity; 5],
    pub(super) container: (Option<f32>, Option<f32>),
}

impl OwnKey {
    pub(crate) fn new(stages: [Identity; 5], container: (Option<f32>, Option<f32>)) -> Self {
        Self { stages, container }
    }

    pub(super) fn same_stages(&self, other: &Self) -> bool {
        self.stages
            .iter()
            .zip(&other.stages)
            .all(|(a, b)| match (a, b) {
                (Some(a), Some(b)) => Weak::ptr_eq(a, b),
                (a, b) => a.is_none() && b.is_none(),
            })
    }
}

/// Document inputs every stage may read.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct DocInputs {
    pub(super) fonts: u64,
    scheme: Scheme,
    clearance: bool,
    root: [u32; 2],
    viewport_known: bool,
}

pub(super) fn doc_inputs(vcx: ValueCx<'_>) -> DocInputs {
    let root = vcx.doc.root.borrow();
    DocInputs {
        fonts: vcx.doc.font_generation(),
        scheme: vcx.scheme,
        clearance: vcx.clearance_possible,
        root: [root.font_px.to_bits(), root.line_px.to_bits()],
        viewport_known: vcx.viewport.0 > 0.0 && vcx.viewport.1 > 0.0,
    }
}

/// The viewport size and the scrollbars viewport units subtract from it.
pub(super) type Viewport = ([u32; 2], [u32; 2]);

pub(super) fn viewport(vcx: ValueCx<'_>) -> Viewport {
    let insets = vcx.doc.viewport.unit_insets();
    (
        [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()],
        [insets[0].to_bits(), insets[1].to_bits()],
    )
}

/// Why a stage was computed again: the first input that differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Miss {
    First,
    Own,
    Container,
    Fonts,
    Document,
    Parent,
    Viewport,
}

impl Miss {
    pub(crate) fn note(self) {
        match self {
            Miss::First => {
                crate::work::add!(BoxStageMissFirst, 1);
            }
            Miss::Own => {
                crate::work::add!(BoxStageMissOwn, 1);
            }
            Miss::Container => {
                crate::work::add!(BoxStageMissContainer, 1);
            }
            Miss::Fonts => {
                crate::work::add!(BoxStageMissFonts, 1);
            }
            Miss::Document => {
                crate::work::add!(BoxStageMissDocument, 1);
            }
            Miss::Parent => {
                crate::work::add!(BoxStageMissParent, 1);
            }
            Miss::Viewport => {
                crate::work::add!(BoxStageMissViewport, 1);
            }
        }
    }
}
