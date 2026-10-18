//! Page table section repetition uses the same reservations as columns.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::layout::fragment::{ShapeCx, table_bands::repeat_bands_in, types::Repeat};
use crate::render::is_blank;
use gpui::AnyElement;

pub(super) fn sections(
    group: &[Node],
    per: f32,
    cx: ShapeCx,
    lead: f32,
    copies: usize,
    build: impl Fn() -> AnyElement,
    vcx: ValueCx<'_>,
) -> Option<Repeat> {
    let mut nodes = group.iter().filter(|n| !is_blank(n));
    let Node::Element(e) = nodes.next()? else {
        return None;
    };
    if nodes.next().is_some() {
        return None;
    }
    let folded_margin = if lead > 0.0 || e.style.float.unwrap_or(0) != 0 {
        crate::layout::fragment::fragment_size::shape_full(e, 4, cx, vcx)?.1
    } else {
        0.0
    };
    let lead = lead + folded_margin;
    let (head, foot, mut geom) = repeat_bands_in(e, per, cx, vcx)?;
    geom.head_end += lead;
    geom.foot_at += lead;
    geom.foot_end += lead;
    geom.box_top += lead;
    geom.box_end += lead;
    Some(Repeat {
        head: head.map(|(at, h)| (at + lead, h)),
        foot: foot.map(|(at, h)| (at + lead, h)),
        geom,
        head_els: head.map_or_else(Vec::new, |_| (1..copies).map(|_| build()).collect()),
        foot_els: foot.map_or_else(Vec::new, |_| (0..copies).map(|_| build()).collect()),
    })
}
