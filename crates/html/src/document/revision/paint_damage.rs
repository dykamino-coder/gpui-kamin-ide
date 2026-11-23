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
use crate::style::computed::Computed;

pub(super) mod leaf;

/// Painted values of one element that changed, as bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PaintValue(u8);

// The layer writes background colour values; other paint deltas require
// command recording until their value bindings land.
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

/// The damage from `a` (old box) to `b` (new box of the same element);
/// `in_svg` tells whether the box is in an `svg` subtree (asked only when
/// its SVG paints changed).
pub(crate) fn classify(a: &Element, b: &Element, in_svg: impl FnOnce() -> bool) -> PaintDamage {
    if a.node_id != b.node_id || a.tag != b.tag || CONTROLS.contains(&a.tag.as_str()) {
        return PaintDamage::Rebuild;
    }
    // The restyle (`Plan::Restyled`) wrote the new paint groups into the
    // boxes in place: the cascaded styles carry the whole difference.
    let (old, new) = (a.style.cascaded(), b.style.cascaded());
    let Some(bits) = own_paint_only(old, new) else {
        return PaintDamage::Rebuild;
    };
    // `fill`, `stroke` and `stroke-width` paint SVG shapes only: in an `svg`
    // subtree they raster the image again (the frame is built); on any other
    // box they paint nothing, and no rule passes them from it into an `svg`
    // (`resources::svg_document::serialize` reads each shape's own value).
    if !old.reset_svg_math.same_paint(&new.reset_svg_math) && in_svg() {
        return PaintDamage::Rebuild;
    }
    if bits.layer_writable() {
        PaintDamage::Value(bits)
    } else {
        PaintDamage::Record(bits)
    }
}

/// A restyle that is not `Rebuild` of the element at `path` paints nothing
/// when the element is SVG content (`svg_content`): its pixels are the image
/// of its `svg`, drawn from its fill and stroke (whose change builds the
/// frame, `classify`), never from the box paint groups left (background,
/// border colours). Counted as `SVG_UNPAINTED`.
pub(super) fn unpainted_svg(nodes: &[crate::dom::Node], path: &[usize]) -> bool {
    let unpainted = svg_content(nodes, path);
    if unpainted {
        crate::paint_path::add(&crate::paint_path::SVG_UNPAINTED, 1);
    }
    unpainted
}

/// A restyle that is not `Rebuild` of the box `a` (new `b`) paints nothing
/// when the element is `display: contents` before and after: it generates no
/// box (css-display-3 §2.5), so its own background and border colours reach
/// no pixel, as Blink gives such an element no `LayoutObject` to invalidate.
/// Inherited values are the same (`own_paint_only`); a descendant that takes
/// a reset value with `inherit` (css-cascade-5 §7.3.1) would paint it, so
/// such a subtree is left to the layer.
pub(super) fn unboxed(a: &Element, b: &Element) -> bool {
    use crate::style::computed::Display;
    let contents = |e: &Element| {
        matches!(
            e.style.cascaded().reset_box_layout.display,
            Some(Display::Contents)
        )
    };
    contents(a)
        && contents(b)
        && super::patch::elements(&b.children, |d, _| !d.style.inherits_reset())
}

/// The element at `path` in `nodes` is SVG content: inside an `svg` element
/// (not that outermost `svg`, a CSS box itself), and neither a
/// `foreignObject` nor under one below that `svg` (HTML boxes again). SVG 2
/// renders such an element into its `svg`'s canvas only (§3 "Rendering
/// Model"), so no box property of CSS reaches it. One descent along the path.
fn svg_content(nodes: &[crate::dom::Node], path: &[usize]) -> bool {
    let mut list = nodes;
    let mut inside = false;
    for (k, &i) in path.iter().enumerate() {
        let Some(crate::dom::Node::Element(e)) = list.get(i) else {
            return false;
        };
        let foreign = e.tag.eq_ignore_ascii_case("foreignObject");
        if k + 1 == path.len() {
            return inside && !foreign;
        }
        if foreign {
            inside = false;
        } else if e.tag == "svg" {
            inside = true;
        }
        list = &e.children;
    }
    false
}

/// The values of the box's own paint `new` changed from `old` when nothing
/// else differs: its background and border colours (`Computed::with_paint_of`, the groups
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
    if old.reset_border.border_colors != new.reset_border.border_colors {
        bits = bits.union(PaintValue::BORDER_COLORS);
    }
    Some(bits)
}
#[cfg(test)]
mod tests {
    use super::{PaintValue, own_paint_only};
    use crate::style::computed::Computed;
    use crate::style::values::value::{Color, Len};

    fn colour() -> Color {
        Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        }
    }

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

    #[test]
    fn each_border_side_requires_a_record_including_a_simultaneous_background_write() {
        let old = Computed::default();
        for side in 0..4 {
            let mut new = old.clone();
            new.reset_border.border_colors[side] = colour().into();
            let border = own_paint_only(&old, &new).unwrap();
            assert_eq!(border, PaintValue::BORDER_COLORS, "side {side}");
            assert!(!border.layer_writable());
            new.reset_background.background = Some(colour());
            let combined = own_paint_only(&old, &new).unwrap();
            assert_eq!(combined, border.union(PaintValue::BACKGROUND_COLOR));
            assert!(!combined.layer_writable());
            assert_eq!(own_paint_only(&new, &old), Some(combined));
        }
    }

    #[test]
    fn unchanged_paint_is_empty_but_inherited_and_layout_changes_are_refused() {
        let old = Computed::default();
        assert_eq!(own_paint_only(&old, &old), Some(PaintValue::default()));
        let mut background = old.clone();
        background.reset_background.background = Some(colour());
        assert_eq!(
            own_paint_only(&old, &background),
            Some(PaintValue::BACKGROUND_COLOR)
        );
        let mut inherited = background.clone();
        inherited.inh.ui.color = Some(colour());
        assert_eq!(own_paint_only(&old, &inherited), None);
        let mut layout = background;
        layout.reset_box_layout.margin.top = Some(Len::Px(1.0));
        assert_eq!(own_paint_only(&old, &layout), None);
    }
}
