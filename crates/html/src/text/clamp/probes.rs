//! Пробы границ коробок только для локального замера ClampHost.

use super::ClampEntry;
use gpui::AnyElement;

/// Проба коробки: Measure сохраняет LayoutId для чтения до освобождения
/// движка; итоговая сборка без замера не создаёт проб.
#[allow(clippy::too_many_arguments)]
pub(crate) fn clamp_probe(
    measure: super::ClampMeasure,
    line: f32,
    skip_count: bool,
    fixed_height: bool,
    bp_after: f32,
    ends: super::line_state::BoxEnds,
    seq: Option<u32>,
    clamped: Option<usize>,
    span: Option<super::ClampSpan>,
    bt_before: f32,
    positioned: bool,
) -> AnyElement {
    let entry = move |bounds| ClampEntry {
        bounds,
        line,
        skip_count,
        fixed_height,
        bp_after,
        end_margin: ends.margin,
        min_end: ends.min_end,
        capped: ends.capped,
        node: ends.node,
        seq,
        clamped,
        empty: false,
        span: span.as_ref().map(|s| s.get()),
        bt_before,
        positioned,
    };
    measure.record(entry)
}

/// Проба пустой поточной блочной коробки клэмп-контейнера: только её
/// положение. Точка среза после неё (§5.3 «a point between two in-flow
/// block-level sibling boxes») отделяет предыдущую строку от точки, и знака
/// обрыва на той строке нет (`line-clamp-auto-039/032`).
pub(crate) fn clamp_empty_probe(measure: super::ClampMeasure) -> AnyElement {
    let entry = move |bounds| ClampEntry {
        bounds,
        line: 0.0,
        skip_count: false,
        fixed_height: false,
        bp_after: 0.0,
        end_margin: 0.0,
        min_end: 0.0,
        capped: false,
        node: 0,
        seq: None,
        clamped: None,
        empty: true,
        span: None,
        bt_before: 0.0,
        positioned: false,
    };
    measure.record(entry)
}
