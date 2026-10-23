//! Повтор шапки и подвала таблицы на страницах: для одиночной таблицы в группе детей
//! корня резервируются те же полосы `Repeat`, что и в колонках (`table_bands`), со
//! сдвигом на схлопнутое поле и копиями элементов полос по числу страниц. У таблицы, чьи
//! ряды даёт раскладка копии, полосы решаются после неё (`page_stack::kids`), а здесь
//! готовятся копии секций, которые по стилю могут повторяться.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::dom::whitespace::is_blank;
use crate::layout::fragment::{ShapeCx, table_bands::repeat_bands_in, types::Repeat};
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
    let folded_margin = if lead > 0.0 || e.style.cascaded().float.unwrap_or(0) != 0 {
        crate::layout::fragment::fragment_size::shape_full(e, 4, cx, vcx)?.margin_top
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
