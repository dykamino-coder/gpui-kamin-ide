//! Форма группы корневых узлов на листе: поля и вырезы.

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::render::is_blank;
use crate::style::computed::Computed;

pub(crate) fn group_shape(
    root: &Computed,
    shape_cx: ShapeCx,
    group: &[Node],
    n: &Node,
    pad_top: f32,
) -> (
    (f32, f32, f32),
    Option<(f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)>,
) {
    let positioned = |e: &Element| {
        matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
    };
    // Поля ребёнка: мера `shape_full` — border box, а обёртка рисует его
    // со смещением на верхнее поле. Прежде поле выбрасывалось, и маска
    // фрагмента высотой в border box резала нарисованное ниже поля
    // (`page-left-right-001-print-ref`: `margin-top: 200px` у блока 100 —
    // жёлтого квадрата не было вовсе). Теперь поля уходят в `Kid.mt/mb`
    // (схлопывание соседей — в `fill`), а копия поднимается на смещение
    // border box внутри обёртки (`PageKid::inner_top`). У первого ребёнка
    // с отбивкой корня и у флоата (его поля не схлопываются, CSS 2.1
    // §8.3.1) поля — часть самой меры.
    let mut margins = (0.0f32, 0.0f32, 0.0f32);
    let shape = match n {
        _ if group.iter().filter(|g| !is_blank(g)).count() > 1 => None,
        Node::Element(e) if !e.inline && !positioned(e) => {
            shape_full(e, 4, shape_cx).map(|(h, mt, mb, mut cuts, mut forced, mut solid)| {
                let floated = e.style.float.unwrap_or(0) != 0;
                // Письмо — своё или унаследованное от корня (`html, body {
                // writing-mode }` снимаются выше, свой стиль ребёнка его не
                // несёт).
                let vertical = e.style.vertical.or(root.vertical) == Some(true);
                let fold = pad_top > 0.0 || floated;
                let lead = if vertical {
                    pad_top
                } else if fold {
                    pad_top + mt
                } else {
                    0.0
                };
                if lead != 0.0 {
                    for c in cuts.iter_mut() {
                        c.0 += lead;
                        c.1 += lead;
                    }
                    for f in forced.iter_mut() {
                        *f += lead;
                    }
                    for r in solid.iter_mut() {
                        r.0 += lead;
                        r.1 += lead;
                    }
                }
                // Вертикальное письмо: поля меры — по блочной оси письма, не
                // по высоте стопки; прежнее поведение (`block-001-wm-vlr/vrl`:
                // `margin-inline-start` сверху — 0.40 -> 0.88 с полями).
                if vertical {
                    let h = if floated { h + mb } else { h };
                    return (h + pad_top, cuts, forced, solid);
                }
                if fold {
                    let tail = if floated { mb } else { 0.0 };
                    margins = (0.0, if floated { 0.0 } else { mb }, 0.0);
                    (h + lead + tail, cuts, forced, solid)
                } else {
                    margins = (mt, mb, mt);
                    (h, cuts, forced, solid)
                }
            })
        }
        _ => None,
    };
    (margins, shape)
}
