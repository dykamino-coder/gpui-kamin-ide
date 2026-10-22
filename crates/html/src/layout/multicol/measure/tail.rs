//! ADR P-19 F2-3: остаток внешнего ряда требует продолжения вложенного fragmentainer.
//! Высота хвостового ряда колонок после единственного измеримого спаннера:
//! остаток блочного размера коробки (css-multicol-1 §column-fill).

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::multicol::spanner::spanner_box;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Хвостовой ряд колонок при `column-fill: auto` НЕ стоит перед
/// спаннером, и §column-fill («content in a multi-column line that
/// does not immediately precede a spanner») велит заполнять его
/// подряд до высоты коробки. Сегмент теряет высоту
/// (`sub.style.height = None`) и уходил в балансировку:
/// `no-balancing-after-column-span` — 200×50 двумя колонками
/// вместо 100×100 одной. Высота хвоста — остаток коробки после
/// спаннера (Blink `ConstrainColumnBlockSize`:
/// `max -= CurrentContentBlockOffset(line_offset)`). Только когда
/// до хвоста стоит ОДИН измеримый спаннер и больше ничего:
/// высоту сбалансированных рядов до спаннера здесь не знаем.
/// То же при балансе: колонки хвоста не выше остатка коробки, лишние
/// переполняют вбок (Blink `ConstrainColumnBlockSize` действует и на
/// балансируемый ряд; `multicol-span-all-012`: 120 в трёх колонках по 30
/// после спаннера 70 в коробке 100).
pub(crate) fn tail_rest_height(
    e: &Element,
    merged: &Computed,
    box_h: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    shape_cx: ShapeCx<'_>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<f32> {
    let is_span = |n: &Node| matches!(n, Node::Element(c) if spanner_box(c));
    let cap_only = rows.is_none_or(|r| r.cap);
    match (box_h, cap_only) {
        (Some(total), true) => e.children.iter().rposition(&is_span).and_then(|j| {
            let px = |l: &Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(*v),
                _ => None,
            };
            let mut used = 0.0f32;
            let mut spans = 0usize;
            for n in &e.children[..=j] {
                if is_blank(n) {
                    continue;
                }
                let Node::Element(sp) = n else {
                    return None;
                };
                if !is_span(n) {
                    return None;
                }
                spans += 1;
                let st = inherit(merged, &sp.style, vcx);
                used += shape_full(sp, 4, shape_cx, vcx)?.height
                    + px(&st.margin.top)?
                    + px(&st.margin.bottom)?;
            }
            (spans == 1).then(|| (total - used).max(0.0))
        }),
        _ => None,
    }
}
