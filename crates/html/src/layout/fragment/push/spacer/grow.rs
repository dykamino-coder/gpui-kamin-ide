//! Монолит ребёнка колонок, держащийся только на break-inside: avoid.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::probe::size_monolith;
use crate::style::computed::Display;

/// Монолит ребёнка колонок держится ТОЛЬКО на `break-inside: avoid`: ни
/// `contain: size`, ни прокрутки, ни замещаемого, ни атомарной строчной, ни
/// сплошного строчного набора (тот же список, что у `monolith` в сборке
/// стопки, без `break_inside_avoid`).
pub(crate) fn avoid_only_monolith(c: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| matches!(n, Node::Element(k) if !k.inline || k.style.used().display == Some(Display::Block));
    !(size_monolith(c)
        || scrolls(c.style.used().overflow_x)
        || scrolls(c.style.used().overflow_y)
        || matches!(
            c.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            c.style.used().display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
        || (c.children.iter().any(|n| !is_blank(n)) && !c.children.iter().any(block_kid)))
}
