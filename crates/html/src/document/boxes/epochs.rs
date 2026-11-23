//! Epochs of the layout inputs of a box: its own, its subtree's and its intrinsic contribution.
//!
//! An epoch is the publication number (`BoxTree::clock`) at which the input changed.
//! A consumer keeps the number it laid out at and compares: nothing is reset.

use crate::dom::Element;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

/// Publication numbers at which the layout inputs of a box changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Epochs {
    /// Own inputs: the used layout style, the attributes layout reads,
    /// marks, inline content, the list of child boxes.
    pub(crate) layout: u32,
    /// A descendant's own inputs changed (set on every ancestor up to the root).
    pub(crate) child: u32,
    /// The box's min/max-content contribution to its ancestor may change.
    pub(crate) intrinsic: u32,
    /// Something the element build of the box or of its subtree reads changed:
    /// layout inputs or paint-only ones (set on every ancestor).
    pub(crate) paint: u32,
}

/// The box's contribution to its ancestor's intrinsic sizes does not depend on its content:
/// the inline size and its min/max are lengths, so are the padding and border, or the box
/// is out of flow. A table cell and caption, a flex/grid item, a table and an
/// orthogonal flow do not stop the propagation: their contribution reads the content
/// (css-tables-3 §3.9, css-flexbox-1 §4.5, Servo
/// `outer_inline_content_sizes_depend_on_content`).
pub(super) fn external_contribution(e: &Element, parent: Option<&Element>) -> bool {
    let style = e.style.used();
    if matches!(
        style.reset_positioned.position,
        Some(Position::Absolute | Position::Fixed)
    ) {
        return true;
    }
    if let Some(parent) = parent {
        let p = parent.style.used();
        let item = matches!(
            p.reset_box_layout.display,
            Some(
                Display::Flex
                    | Display::InlineFlex
                    | Display::Grid
                    | Display::InlineGrid
                    | Display::GridLanes
            )
        );
        if item || p.inh.writing.writing_mode != style.inh.writing.writing_mode {
            return false;
        }
    }
    let layout = &style.reset_box_layout;
    // `display: inline` after the cascade is `InlineBlock` with `inline_display`
    // (`layout::box_kind::inline_level_box`); an inline box has no width.
    let floated = layout.float.is_some_and(|f| f != 0);
    let inline = !floated
        && (style.reset_context.inline_display == Some(true)
            || layout.display.is_none() && e.inline);
    let boxed = matches!(
        layout.display,
        None | Some(
            Display::Block
                | Display::ListItem
                | Display::InlineBlock
                | Display::Flex
                | Display::InlineFlex
                | Display::Grid
                | Display::InlineGrid
                | Display::GridLanes
        )
    );
    // A cell and a table: their width is only a lower bound (css-tables-3 §3.9);
    // table display types of HTML elements come from the tag, not from `display`.
    let table = matches!(
        e.tag.as_str(),
        "table" | "caption" | "td" | "th" | "tr" | "tbody" | "thead" | "tfoot"
    );
    if inline || !boxed || table {
        return false;
    }
    let fixed = |len: &Option<Len>| matches!(len, Some(Len::Px(_) | Len::Em(_) | Len::EmPx(..)));
    let bound = |len: &Option<Len>| len.is_none() || fixed(len) || matches!(len, Some(Len::Auto));
    let sides = |s: &crate::style::computed::Sides| {
        [&s.left, &s.right]
            .into_iter()
            .all(|l| l.is_none() || fixed(l))
    };
    fixed(&layout.width)
        && bound(&layout.min_width)
        && bound(&layout.max_width)
        && layout.calc_size.iter().all(Option::is_none)
        && sides(&layout.padding)
        && sides(&style.reset_border.border_width)
}

/// The box sizes its children from all their intrinsic contributions: a
/// table's columns, a flex line, grid tracks (Blink lays out every child of
/// such a box again when one child's intrinsic sizes change). A table of fixed
/// layout takes its columns from its width and its first row's styles, not
/// from the content of its cells (CSS 2.1 §17.5.2.1; fixed layout applies
/// only under a non-auto width, css-tables-3 §table-layout-property), as in
/// Blink's `ComputeColumnInlineConstraints` for a fixed table.
pub(super) fn distributes(e: &Element) -> bool {
    let style = e.style.used();
    let fixed = style.reset_context.table_fixed == Some(true)
        && matches!(
            style.reset_box_layout.width,
            Some(Len::Px(_) | Len::Pct(_) | Len::Em(_) | Len::EmPx(..))
        );
    (crate::layout::fragment::table_bands::table_box(e) && !fixed)
        || matches!(
            style.reset_box_layout.display,
            Some(
                Display::Flex
                    | Display::InlineFlex
                    | Display::Grid
                    | Display::InlineGrid
                    | Display::GridLanes
            )
        )
}

/// Attributes layout does not read: class and style arrive through the
/// used style, the others serve only the cascade, links and accessibility.
fn layout_reads(name: &str) -> bool {
    !matches!(name, "class" | "style" | "id" | "title" | "href")
        && !name.starts_with("aria-")
        && !name.starts_with("data-")
}

/// Equal own layout inputs of a box in two publications (without children).
/// Unlike `dom::same_layout_box` (the record replay key, which also replays
/// the paint of SVG markup), only layout matters here. `inherited`: the
/// parents have the same computed styles (all ancestors passed this comparison), and
/// the style of a rebuild not yet computed is compared by its cascade
/// (`dom::same_cascaded_layout_stages`, Blink `StyleDifference` before layout).
pub(super) fn same_layout_inputs(a: &Element, b: &Element, inherited: bool) -> bool {
    let read = |(name, _): &&(String, String)| layout_reads(name);
    a.node_id == b.node_id
        && a.tag == b.tag
        && a.inline == b.inline
        && a.list_item == b.list_item
        && match (&a.anim, &b.anim) {
            (None, None) => true,
            (Some(x), Some(y)) => std::rc::Rc::ptr_eq(x, y),
            _ => false,
        }
        && a.marks == b.marks
        && (a.attrs == b.attrs || a.attrs.iter().filter(read).eq(b.attrs.iter().filter(read)))
        && if inherited {
            crate::dom::same_cascaded_layout_stages(a, b)
        } else {
            crate::dom::same_layout_stages(a, b)
        }
}
