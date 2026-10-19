//! Measured clamp paragraph rows and layout probes.

use super::{ClampEntry, ClampLines};
use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled};

thread_local! {
    /// Настоящие строки абзацев клэмп-контейнера на этом кадре: (ключ,
    /// номер абзаца) → (верх, низ) строк в координатах окна. Пишет их
    /// отрисовка абзаца (`lines::Paragraph::paint`), забирает `ClampCut`
    /// (он рисуется последним ребёнком контейнера). Без них строки
    /// абзаца делились бы поровну, а строка с крупным кеглем или руби
    /// выше прочих (css-overflow-4 §5.3: точка среза — между строчными
    /// коробками, их высоты свои).
    #[allow(clippy::type_complexity)] // A container/paragraph key maps to measured row bounds.
    pub(super) static PARA_ROWS: std::cell::RefCell<std::collections::HashMap<(u64, u32), Vec<(f32, f32)>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Отрисовка абзаца сообщает его строки (см. `PARA_ROWS`).
pub fn publish_para_rows(tag: (u64, u32), rows: Vec<(f32, f32)>) {
    PARA_ROWS.with(|m| {
        m.borrow_mut().insert(tag, rows);
    });
}

pub(super) fn take_para_rows(key: u64) -> std::collections::HashMap<u32, Vec<(f32, f32)>> {
    PARA_ROWS.with(|m| {
        let mut m = m.borrow_mut();
        let tags: Vec<(u64, u32)> = m.keys().filter(|k| k.0 == key).copied().collect();
        tags.into_iter()
            .filter_map(|t| {
                m.remove(&t).map(|mut v| {
                    v.sort_by(|a, b| a.0.total_cmp(&b.0));
                    (t.1, v)
                })
            })
            .collect()
    })
}

/// Проба строк: канвас в элементе с текстом (или блоке), пишет границы и
/// высоту строки в prepaint своего кадра.
// The probe records paragraph identity, line geometry and clamp constraints.
#[allow(clippy::too_many_arguments)]
pub fn clamp_probe(
    lines: ClampLines,
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
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
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
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Проба пустой поточной блочной коробки клэмп-контейнера: только её
/// положение. Точка среза после неё (§5.3 «a point between two in-flow
/// block-level sibling boxes») отделяет предыдущую строку от точки, и знака
/// обрыва на той строке нет (`line-clamp-auto-039/032`).
pub fn clamp_empty_probe(lines: ClampLines) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
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
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}
