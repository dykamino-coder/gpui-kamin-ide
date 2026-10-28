//! Элементы потоков flex у края фрагмента: подавленный промежуток, начала рядов и линий.

use super::Growth;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid, Pushed};
use crate::tolerance::COORD_EPS;

/// Элемент колонки, начинающийся за краем фрагментаинера после содержимого
/// этого фрагмента: разрыв перед ним, а промежуток между ними (row-gap и место
/// `justify-content`), не поместившийся до края, гасится — элемент встаёт на
/// начало следующего фрагментаинера (Blink
/// `UpdateOffsetAdjustmentForSuppressedRowGap`, эффективный промежуток
/// `effective_gap_between_items`). Сдвиг отрицательный; заданный размер
/// контейнера он не меняет.
pub(super) fn pushed_past_edge(
    flow: &BlockBox,
    laid: &Laid,
    from: f32,
    edge: f32,
) -> Option<Growth> {
    flow.flex_gap?;
    let mut prev_end: Option<f32> = None;
    for kid in &flow.kids {
        let (top, bottom) = laid.margin_box(kid.id).or_else(|| laid.at(kid.id))?;
        if top >= edge - COORD_EPS {
            let ended = prev_end.is_some_and(|e| e > from + COORD_EPS && e <= edge + COORD_EPS);
            if ended && top > edge + COORD_EPS {
                return Some(Growth {
                    target: Pushed::Box(kid.id),
                    at: top,
                    amount: edge - top,
                    forced: false,
                });
            }
            return None;
        }
        prev_end = Some(bottom);
    }
    None
}

/// Верх элемента ряда flex или элемента сетки, чей поток — `id`: его первую неделимую часть
/// переносит линия или ряд (`FlexBox::breaks`, `GridBox::breaks`).
pub(super) fn row_item_top(laid: &Laid, id: u64) -> Option<f32> {
    (laid.placements.contains_key(&id)
        || laid
            .flexes
            .values()
            .any(|f| !f.column && f.lines.iter().any(|l| l.items.contains(&id))))
    .then(|| laid.at(id).map(|p| p.0))
    .flatten()
}

/// Начало линии ряда, в которой стоит элемент `id`.
pub(super) fn row_item_line_start(laid: &Laid, id: u64) -> Option<f32> {
    laid.flexes
        .values()
        .filter(|f| !f.column)
        .flat_map(|f| &f.lines)
        .find(|l| l.items.contains(&id))
        .map(|l| l.start)
}

/// Элемент колонки flex: сдвигается своим смещением, а не распоркой.
pub(super) fn column_item(laid: &Laid, id: u64) -> bool {
    laid.flexes
        .values()
        .any(|f| f.column && f.lines.iter().any(|l| l.items.contains(&id)))
}

/// Открытый ряд (`growths`, `open`): линия ряда flex с верха колонки не переносится,
/// и неделимый элемент этой линии (монолит или `break-inside: avoid`), не
/// вмещающийся до края, переносится сам — ранний разрыв перед элементом (Blink).
pub(super) fn open_row_items(
    b: &BlockBox,
    laid: &Laid,
    from: f32,
    edge: f32,
    out: &mut Vec<Growth>,
) {
    if let Some(flex) = &b.flex
        && laid.flexes.get(&b.id).is_some_and(|f| !f.column)
    {
        for item in &flex.items {
            let Some((top, bottom)) = laid.at(item.id) else {
                continue;
            };
            let whole = item.monolith || item.inner.as_ref().is_some_and(|i| i.inside_avoid);
            if whole && (top - from).abs() < COORD_EPS && bottom > edge + COORD_EPS {
                out.push(Growth {
                    target: Pushed::Box(item.id),
                    at: top,
                    amount: edge - top,
                    forced: false,
                });
            }
        }
    }
    let flex_items = b.flex.iter().flat_map(|f| &f.items);
    for inner in b
        .kids
        .iter()
        .chain(flex_items)
        .filter_map(|k| k.inner.as_ref())
    {
        open_row_items(inner, laid, from, edge, out);
    }
}
