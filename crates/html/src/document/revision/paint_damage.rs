//! Paint-only damage of one restyled element: which painted values changed
//! (`PaintValue`, a set of bits), or that the box's own commands must be
//! recorded again (`Record`), or that the frame must be built (`Rebuild`).
//!
//! It is the paint side of a restyle's outcome (design-incremental §2.5,
//! ST3 `RestyleOutcome { elem, damage, subtree_inherit }`). A `Value` write
//! goes into the document's scene layer between frames (`super::layer_write`);
//! a `Record` repaints the box's run of commands in the layer on the next
//! frame (`gpui::Window::rerecord_layer_box`), any number of them; neither
//! builds the element tree. Until ST3 reports the damage itself, `classify`
//! derives it from the old and new styles of `Plan::Restyled`.

use crate::dom::Element;
use crate::style::computed::{BoxStyle, Computed};

/// Painted values of one element that changed, as bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PaintValue(u8);

// The bits other than the background colour are the contract ST3 and R3
// fill; the layer takes them as their bindings land.
#[allow(dead_code)]
impl PaintValue {
    pub(crate) const BACKGROUND_COLOR: Self = Self(1);
    pub(crate) const BORDER_COLORS: Self = Self(1 << 1);
    pub(crate) const COLOR: Self = Self(1 << 2);
    pub(crate) const OUTLINE_COLOR: Self = Self(1 << 3);
    pub(crate) const TEXT_DECORATION_COLOR: Self = Self(1 << 4);
    pub(crate) const OPACITY: Self = Self(1 << 5);
    /// The values the scene layer takes by a write (`LayerHandle`).
    pub(crate) const LAYER_WRITABLE: Self = Self::BACKGROUND_COLOR;

    pub(crate) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Every changed value is one the layer takes by a write.
    pub(crate) fn layer_writable(self) -> bool {
        Self::LAYER_WRITABLE.contains(self)
    }
}

/// What a restyle of one element asks of paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaintDamage {
    /// Only these painted values changed: layout and commands hold.
    Value(PaintValue),
    /// Only the box's own commands change (in number too): layout holds.
    Record(PaintValue),
    /// Anything else: the frame is built.
    Rebuild,
}

/// Elements `paint::controls` paints with UA defaults: a declared
/// background or border colour is new paint around them.
pub(super) const CONTROLS: [&str; 6] =
    ["input", "button", "select", "textarea", "meter", "progress"];

/// The damage from `a` (old box) to `b` (new box of the same element).
pub(crate) fn classify(a: &Element, b: &Element) -> PaintDamage {
    if a.node_id != b.node_id || a.tag != b.tag || CONTROLS.contains(&a.tag.as_str()) {
        return PaintDamage::Rebuild;
    }
    // The used style is the cascaded one unless layout edited it: then both
    // must differ the same way.
    let (Some(bits), Some(_)) = (
        own_paint_only(a.style.cascaded(), b.style.cascaded()),
        own_paint_only(
            &BoxStyle::shared(a.style.used()),
            &BoxStyle::shared(b.style.used()),
        ),
    ) else {
        return PaintDamage::Rebuild;
    };
    if bits.layer_writable() {
        PaintDamage::Value(bits)
    } else {
        PaintDamage::Record(bits)
    }
}

/// The values of the box's own paint `new` changed from `old` when nothing
/// else differs: its background (`Computed::with_paint_of`, the paint groups
/// of a restyle). `None` for any other difference, a new inherited UI group
/// (`color` paints the text of descendants) included.
fn own_paint_only(old: &Computed, new: &Computed) -> Option<PaintValue> {
    if old.inh.ui != new.inh.ui || old.with_paint_of(new) != *new {
        return None;
    }
    let mut bits = PaintValue::default();
    if old.reset_background != new.reset_background {
        bits = bits.union(PaintValue::BACKGROUND_COLOR);
    }
    Some(bits)
}
#[cfg(test)]
mod tests {
    use super::PaintValue;

    #[test]
    fn only_the_background_colour_is_written_into_the_layer() {
        assert!(PaintValue::BACKGROUND_COLOR.layer_writable());
        assert!(PaintValue::default().layer_writable());
        for other in [
            PaintValue::BORDER_COLORS,
            PaintValue::COLOR,
            PaintValue::OUTLINE_COLOR,
            PaintValue::TEXT_DECORATION_COLOR,
            PaintValue::OPACITY,
        ] {
            assert!(!other.layer_writable());
            assert!(!PaintValue::BACKGROUND_COLOR.union(other).layer_writable());
            assert!(!other.is_empty());
        }
    }
}
