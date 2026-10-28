//! Поля группы корневых узлов листа, чью высоту и разрывы даёт раскладка её копии.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::cx::FrameCx;
use crate::style::computed::BoxStyle;

/// Поля ребёнка листа: копия рисуется со смещением на верхнее поле, и маска
/// фрагмента высотой в border box иначе резала бы нарисованное ниже поля
/// (`page-left-right-001-print-ref`: `margin-top: 200px` у блока 100). Поля
/// уходят в `Kid.mt/mb` (схлопывание соседей — в `fill`), копия поднимается на
/// смещение border box (`PageKid::inner_top`). У первого ребёнка с отбивкой
/// корня поле складывается в обёртку и не схлопывается с краем листа; у флоата
/// поля не схлопываются (CSS 2.1 §8.3.1) и принадлежат его margin box.
pub(crate) fn group_margins(
    root: &BoxStyle,
    group: &[Node],
    n: &Node,
    pad_top: f32,
    vcx: FrameCx<'_>,
) -> (f32, f32, f32) {
    let positioned = |e: &Element| {
        matches!(
            e.style.used().position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
    };
    match n {
        _ if group.iter().filter(|g| !is_blank(g)).count() > 1 => (0.0, 0.0, 0.0),
        Node::Element(e) if !e.inline && !positioned(e) => {
            if e.style.used().float.is_some_and(|f| f != 0) {
                return (0.0, 0.0, 0.0);
            }
            let e = crate::layout::fragment::line_shape::resolved_lengths(e, root, vcx);
            let Some((mt, mb)) = crate::layout::block::margins::fragment_margins(&e) else {
                return (0.0, 0.0, 0.0);
            };
            if pad_top > 0.0 {
                (0.0, mb, 0.0)
            } else {
                (mt, mb, mt)
            }
        }
        _ => (0.0, 0.0, 0.0),
    }
}
