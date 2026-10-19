//! Paragraph rows for clamp; split out to keep the owning module within 250 lines.

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
    /// (ключ, номер) абзаца, который сейчас будет собран.
    pub(super) static PARA_TAG: std::cell::Cell<Option<(u64, u32)>> = const { std::cell::Cell::new(None) };
}

thread_local! {
    /// How many atomic inlines are being built around the current point
    /// of the tree build (see `ParaNest`).
    static PARA_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Paragraphs built inside an atomic inline of another paragraph (ruby
/// bases and annotations, inline-block contents) lie INSIDE a line box of
/// that paragraph: their own lines are not line boxes of the clamp
/// container's BFC (css-overflow-4 §5.3), so they get no clamp number, no
/// probe and no line budget.
pub struct ParaNest(u32);

impl ParaNest {
    pub fn enter() -> Self {
        ParaNest(PARA_DEPTH.with(|d| d.replace(d.get() + 1)))
    }

    /// An atomic inline that is itself a line-clamp container
    /// (`-webkit-inline-box`, inline-block with `line-clamp`) counts its
    /// own lines: the nesting restarts inside it.
    pub fn reset() -> Self {
        ParaNest(PARA_DEPTH.with(|d| d.replace(0)))
    }
}

impl Drop for ParaNest {
    fn drop(&mut self) {
        PARA_DEPTH.with(|d| d.set(self.0));
    }
}

/// Replace the atomic-inline nesting depth, returning the old one.
pub(super) fn swap_para_depth(v: u32) -> u32 {
    PARA_DEPTH.with(|d| d.replace(v))
}

/// Number of enclosing atomic-inline builds (`ParaNest`).
pub fn para_depth() -> u32 {
    PARA_DEPTH.with(std::cell::Cell::get)
}

pub fn set_para_tag(v: Option<(u64, u32)>) {
    PARA_TAG.with(|c| c.set(v));
}

pub fn take_para_tag() -> Option<(u64, u32)> {
    PARA_TAG.with(|c| c.take())
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

thread_local! {
    /// Бюджет строк ТЕКУЩЕГО собираемого абзаца (только авто-режим).
    pub(super) static PARA_BUDGET: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Положить бюджет строк для абзаца, который сейчас будет собран.
pub fn set_para_budget(v: Option<usize>) {
    PARA_BUDGET.with(|c| c.set(v));
}

/// Забрать бюджет (и опустошить ячейку). Опустошение обязательно: куски
/// абзаца строят вложенные абзацы (`inline-block`), и им чужой бюджет
/// доставаться не должен.
pub fn take_para_budget() -> Option<usize> {
    PARA_BUDGET.with(|c| c.take())
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
