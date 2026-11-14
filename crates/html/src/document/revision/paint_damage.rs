//! Paint-only damage of one restyled element: which painted values changed
//! (`PaintValue`, a set of bits) or that its paint must be recorded again
//! (`PaintDamage::Record`).
//!
//! It is the paint side of a restyle's outcome (design-incremental §2.5,
//! ST3 `RestyleOutcome { elem, damage, subtree_inherit }`): a `Value` write
//! goes into the document's scene layer without a frame
//! (`super::layer_write`); a `Record` (any other change) builds the frame as
//! before. Until ST3 reports the damage itself, `classify` derives it from the
//! old and new styles of `Plan::Restyled`; R1 writes the background colour,
//! the other bits are named so the layer write grows by one binding each.

use crate::dom::Element;
use crate::style::computed::Computed;

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
    /// Only these painted values changed: layout and paint commands hold.
    Value(PaintValue),
    /// Anything else changed: the element's paint is recorded again.
    Record,
}

/// The damage from `a` (old box) to `b` (new box of the same element).
pub(crate) fn classify(a: &Element, b: &Element) -> PaintDamage {
    if a.node_id != b.node_id || a.tag != b.tag {
        return PaintDamage::Record;
    }
    let (old, new) = (a.style.cascaded(), b.style.cascaded());
    let Some(own) = background_only(old, new) else {
        return PaintDamage::Record;
    };
    // A form control paints a UA fill and corners only while the author
    // declares no background (`paint::controls`): a declaration is new paint.
    let declared = old.reset_background.bg_explicit != new.reset_background.bg_explicit;
    if declared && CONTROLS.contains(&a.tag.as_str()) {
        return PaintDamage::Record;
    }
    // Layout's used layer (when it edited the style) must differ the same way.
    match (a.style.used_rc(), b.style.used_rc()) {
        (None, None) => PaintDamage::Value(own),
        (Some(x), Some(y)) if background_only(&x, &y).is_some() => PaintDamage::Value(own),
        _ => PaintDamage::Record,
    }
}

/// Elements `paint::controls` paints with UA defaults.
const CONTROLS: [&str; 6] = ["input", "button", "select", "textarea", "meter", "progress"];

/// `BACKGROUND_COLOR` (or no bit) when `new` differs from `old` only in its
/// own background colour (and whether the author declared one); `None` for
/// any other difference.
fn background_only(old: &Computed, new: &Computed) -> Option<PaintValue> {
    let (a, b) = (&old.reset_background, &new.reset_background);
    let same_colour = a.background == b.background && a.background_color == b.background_color;
    let mut probe = old.clone();
    let group = &mut probe.reset_background;
    group.background = b.background;
    group.background_color = b.background_color.clone();
    group.bg_explicit = b.bg_explicit;
    if probe != *new {
        return None;
    }
    Some(if same_colour {
        PaintValue::default()
    } else {
        PaintValue::BACKGROUND_COLOR
    })
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
