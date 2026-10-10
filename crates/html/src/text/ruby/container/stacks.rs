//! Stacks for container; split out to keep the owning module within 250 lines.

use gpui::{AnyElement, ParentElement, Styled, div};

// Над базой — сетка 1×1: база и обёртка уровней делят одну
// ячейку. Первая базовая сетки — у первого по порядку элемента
// первого ряда (css-grid-2 §10.7 «grid baselines»: `row-major
// grid order`), то есть у базы. Прежняя `column-reverse` отдавала
// базовую линию ВИЗУАЛЬНО начального элемента (css-flexbox-1
// §8.5 «startmost flex item», Taffy 0.14) — обёртки уровней,
// а не базы (`ruby-align-001`, `empty-ruby-text-container-float`).
pub(super) fn over_stack(base: AnyElement) -> gpui::Div {
    div()
        .grid()
        .flex_shrink_0()
        .grid_template_cols(vec![gpui::GridTrack::Auto])
        .grid_template_rows(vec![gpui::GridTrack::Auto])
        .child(div().row_start(1).col_start(1).child(base))
}

// Уровни аннотаций уходят из БЛОЧНОГО потока колонки (css-ruby-1
// §3.4: «ordinarily, ruby annotation containers and ruby
// annotation boxes do not contribute to the measured height of a
// line's inline contents»). Обёртка нулевой ГЛАВНОЙ высоты:
// `h_0` задаёт основу, `min_h_0` снимает автоминимум гибкого
// элемента (`vendor/taffy/src/compute/flexbox.rs:824-827`: иначе
// `min-height: auto` вернёт высоту содержимого). Над базой
// содержимое прижато к главному концу — свободное место
// отрицательное, и `FlexEnd` отдаёт его целиком
// (`compute/common/alignment.rs:61-67`), уровни встают НАД нулём;
// под базой обычный `flex-start` свисает вниз.
//
// Зачем: первую базовую линию гибкой КОЛОНКИ taffy берёт у
// первого DOM-ребёнка (`flexbox.rs:405-419`), а `child.baseline`
// колонки (`:2152`) содержит `total_offset_main`, который в
// `column-reverse` равен ВЫСОТЕ аннотаций. Пока уровни лежали в
// самой колонке, атом отдавал строке базовую линию на H(ann)
// ниже: замерено `target/ruby-probe/probe-c.html` (Ahem 64px) —
// 88 dev вместо 104/144/184 при `line-height` 1/2/3, и
// `probe-d.html` — 128 вместо 144 при `rt { font-size: 64px }`,
// 56 вместо 72 при `rb { font-size: 32px }`. На живой паре
// `text-box-trim-ruby-start-001` (Ahem 40px, H(ann) = 25 dev) это
// давало строку на 15 dev ниже эталона и базу на 25 dev ниже.
pub(super) fn level_wrap(under: bool) -> gpui::Div {
    let w = div().flex().flex_col().flex_shrink_0().h_0().min_h_0();
    if under {
        w
    } else {
        // Над базой: та же ячейка сетки `over_stack`, прижатая к
        // её верху; уровни свисают вверх от нуля.
        let mut w = w.justify_end().row_start(1).col_start(1);
        w.style().align_self = Some(gpui::AlignItems::Start);
        w
    }
}

pub(super) fn align_units(merged: &mut crate::style::computed::Computed) {
    use crate::style::computed::{RubyAlign, TextAlign};
    // `ruby-align`: `space-around` (начальное) и `center` — по центру
    // (у латиницы точек выключки нет, §4.3); `space-between` —
    // выключка обоих краёв; `start` — к началу.
    match merged.ruby_align {
        Some(RubyAlign::Start) => merged.text_align = Some(TextAlign::Start),
        Some(RubyAlign::SpaceBetween) => {
            merged.text_align = Some(TextAlign::Justify);
            merged.text_align_last = Some(TextAlign::Justify);
        }
        _ => merged.text_align = Some(TextAlign::Center),
    }
}
