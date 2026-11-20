//! Which blocks may stand far from the screen as their exact leaf only.
//!
//! The leaf replaces the block's whole subtree in the frame tree, so the
//! subtree must reach nothing outside its own box: no positioned or own-layer
//! box (it would paint or be placed by an outer containing block), no float
//! escaping it (it would push the lines of later siblings), no scroller or
//! control (element state that lives only while the element is built), no
//! animation (it asks for frames). The block itself is a plain block-level box
//! whose border box is its parent's stretch fit (`width: auto`, no limits) and
//! whose margins and padding do not depend on the containing block's width,
//! so the leaf requested from its margins stands exactly where it would.

use crate::dom::{Element, Node};
use crate::style::computed::{Display, Overflow, Position};
use crate::style::values::value::Len;

/// Elements whose state or frames live only while they are built.
const STATEFUL: &[&str] = &[
    "input",
    "textarea",
    "select",
    "button",
    "video",
    "audio",
    "iframe",
    "canvas",
    "details",
    "summary",
    "dialog",
    "object",
    "embed",
    "marquee",
    "html-viewport",
];

/// Non-whitespace characters of the text of `e`'s subtree, when `e` may
/// stand far from the screen as its leaf; `None` otherwise.
pub(super) fn far_leaf_text(e: &Element) -> Option<usize> {
    let style = e.style.used();
    let layout = &style.reset_box_layout;
    let fixed = |l: Option<Len>| !matches!(l, Some(Len::Pct(_)));
    let sides = |s: &crate::style::computed::Sides| {
        fixed(s.top) && fixed(s.right) && fixed(s.bottom) && fixed(s.left)
    };
    let own = matches!(layout.display, None | Some(Display::Block))
        && matches!(layout.width, None | Some(Len::Auto))
        && matches!(
            layout.min_width,
            None | Some(Len::Auto) | Some(Len::Px(0.0))
        )
        && matches!(layout.max_width, None)
        && sides(&layout.margin)
        && sides(&layout.padding)
        && !e.style.used().wm().vertical()
        && e.first_line.is_none()
        && e.first_letter.is_none()
        && !super::super::plain::escaping_floats(e);
    if !own {
        return None;
    }
    let mut chars = 0;
    let quiet = |n: &Element| {
        let s = n.style.used();
        let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
        matches!(s.reset_positioned.position, None | Some(Position::Static))
            && !crate::style::computed::stacking::stacking_context(&s)
            && visible(s.reset_box_layout.overflow_x)
            && visible(s.reset_box_layout.overflow_y)
            && n.anim.is_none()
            && !STATEFUL.contains(&n.tag.as_str())
    };
    let all = super::super::plain::all_elements(e, |n| {
        chars += own_text(n);
        quiet(n)
    });
    all.then_some(chars)
}

/// Non-whitespace characters of the text children of `e`.
pub(super) fn own_text(e: &Element) -> usize {
    let text = e.children.iter().map(|c| match c {
        Node::Text(t) => t.chars().filter(|c| !c.is_whitespace()).count(),
        Node::Element(_) => 0,
    });
    text.sum()
}
