//! Box changes from one publication, independent of paint refusal reasons.
//!
//! This is a publication result, not a frame queue. A BoxId may occur multiple
//! times: consumers merge independent damage without losing paint when handling
//! layout. Retired keys identify the former arena generation.

use crate::dom::BoxId;
pub(crate) use crate::dom::source_delta::{SourceChange, SourceChangeKind, SourceDelta};

/// Changed layout inputs; Content respects external intrinsic constraints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LayoutDamage {
    #[default]
    None,
    Content,
    Own,
}

/// Independent stage requirements; structural damage does not replace the document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Damage {
    pub(crate) structure: bool,
    pub(crate) layout: LayoutDamage,
    pub(crate) paint: bool,
}

impl Damage {
    pub(crate) const PAINT: Self = Self {
        structure: false,
        layout: LayoutDamage::None,
        paint: true,
    };
    pub(crate) const CONTENT: Self = Self {
        layout: LayoutDamage::Content,
        ..Self::PAINT
    };
    pub(crate) const LAYOUT: Self = Self {
        layout: LayoutDamage::Own,
        ..Self::PAINT
    };
    pub(crate) const STRUCTURE: Self = Self {
        structure: true,
        ..Self::LAYOUT
    };
    pub(crate) const CHILDREN: Self = Self {
        structure: true,
        ..Self::CONTENT
    };
}

/// Identifiers do not retain the source DOM, adapters or GPUI elements.
#[derive(Debug, Default)]
pub(crate) struct BoxDelta {
    pub(crate) changed: Vec<(BoxId, Damage)>,
    pub(crate) retired: Vec<BoxId>,
}

impl BoxDelta {
    pub(crate) fn mark(&mut self, id: BoxId, damage: Damage) {
        self.changed.push((id, damage));
    }

    pub(crate) fn retire(&mut self, id: BoxId) {
        self.retired.push(id);
    }
}
