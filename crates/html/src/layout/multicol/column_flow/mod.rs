//! Текстовый поток колонок: многоколоночник из сплошного строчного содержимого режется по
//! строкам самого текста (`ColumnFlow` из `float::split_flow`), а не стопкой блоков.
//! Здесь собираются входы разрезчика: нормализованный текст и карта его байтов на сырые
//! узлы, кегль и интерлиньяж, вычисленный `column-gap`, линейка колонок, высота заливки
//! для `column-fill: auto` и `orphans`/`widows`; содержимое с блоками уходит в `blocks`.

use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::multicol::dimensions::MulticolDimensions;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use crate::text::fonts::style_font::measure_font;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, SharedString};
mod text;
use text::{build_text_columns, css_ws, gather_cols, normalized_text};
mod blocks;
use blocks::non_inline_columns;
mod element;
mod layout;
pub(crate) use element::ColumnFlow;
mod balance;
mod intrinsic;
mod measure;
mod rows;
mod rule;
use crate::layout::fragment::token::space::sequence::Nested;
use std::rc::Rc;

mod sizing;
pub(crate) use sizing::{column_flow, column_flow_with_dimensions};

/// `whole` — текст пришёл рекурсией из единственного ребёнка-МОНОЛИТА
/// (`size_monolith`; css-contain-2 §containment-size: «Size containment boxes
/// are monolithic», Blink `layout_box.cc:3564-3575` `IsMonolithic`). Строки
/// монолита между колонками не расходятся; в узкой колонке `measure_columns`
/// держит для него прежний сторож «без разрезов» (`contain-size-breaks-001`).
// Сборщик колонки сохраняет все метрики для построения фрагмента.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn column_flow_in(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    whole: bool,
    stretch: Option<f32>,
    outer: Option<Rc<Nested>>,
    dimensions: MulticolDimensions,
) -> Option<AnyElement> {
    // Позиционированный строчный ребёнок: его содержащий блок — не колонка,
    // а предок (css-position-3 §containing-block; Blink кладёт OOF во
    // фрагментаинеры от СВОЕГО содержащего блока,
    // `OutOfFlowLayoutPart::LayoutFragmentainerDescendants`). Текстовый путь
    // знает только строки — такой поток идёт стопкой со статическими местами.
    if e.children.iter().any(|n| {
        matches!(n, Node::Element(c) if c.inline && matches!(
            c.style.used().position,
            Some(crate::style::computed::Position::Absolute | crate::style::computed::Position::Fixed)
        ))
    }) {
        return None;
    }
    let all_inline = e.children.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(child) => child.inline && child.style.used().display.is_none(),
    });
    if let Some(value) = non_inline_columns(
        e,
        inherited,
        opts,
        count,
        col_w,
        whole,
        stretch,
        all_inline,
        outer.clone(),
    ) {
        return value;
    }
    let mut raw_plain = String::new();
    gather_cols(&e.children, &mut raw_plain);
    // Разрезы приходят в байтах НОРМАЛИЗОВАННОЙ строки (по ней меряет
    // line_wrapper), а split_nodes режет сырые узлы по сырым байтам —
    // без обратного маппинга разрез уезжает на длину схлопнутых пробелов.
    // Пробелы вокруг жёсткого разрыва схлопываются в него (css-text §4.1.2).
    let (normalized, norm_to_raw) = normalized_text(&raw_plain);
    let plain = normalized.trim_matches(css_ws).to_string();
    if plain.trim_matches('\n').is_empty() {
        return None;
    }
    let lead = normalized.len() - normalized.trim_start_matches(css_ws).len();
    let raw_len = raw_plain
        .chars()
        .filter(|c| *c != '\u{2028}')
        .map(|c| c.len_utf8())
        .sum::<usize>();
    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    let line = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => size * k,
        _ => crate::text::metrics::normal_px(normal_fraction(inherited, opts), size, opts.cx.vcx()),
    };
    // Промежуток — ВЫЧИСЛЕННОЕ значение (css-values-4 §5): `em` в точки переводит
    // `Computed::resolve_em`, а он живёт в наследовании, то есть в `inherited`
    // (`merged` хоста у вызовов из `element()`), но НЕ в `e.style`. Пока читалось
    // `e.style`, `column-gap: 10em` подменялся кеглем (`multicol-gap-000`: вторая
    // колонка на 310 вместо 400; `-gap-fraction-001`, `-gap-large-002`). Стопка
    // читает `merged` с пакета B (`used_gap`), текстовый путь — нет.
    // Только чтение `inherited`: без переноса через рекурсию (там `inherited` —
    // стиль РЕБЁНКА, его промежуток пуст, значение прежнее) и без высоты.
    // `column-gap: -1em` доезжает `Px(-20)`: промежуток `[0,∞]` (css-align-3
    // §column-row-gap), отрицательный невалиден — умолчание
    // (`multicol-gap-negative-001`).
    let gap = match inherited.column_gap.or(e.style.used().column_gap) {
        Some(Len::Px(v)) if v >= 0.0 => v,
        _ => size,
    };
    let rule_owned = rule::column_rule(e, inherited, size);
    let nodes = e.children.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let build: std::rc::Rc<dyn Fn(&[usize], usize, gpui::Pixels, &[rows::Row]) -> AnyElement> =
        std::rc::Rc::new(
            move |cuts: &[usize], used: usize, width: gpui::Pixels, rows: &[rows::Row]| {
                build_text_columns(
                    stretch,
                    &norm_to_raw,
                    lead,
                    raw_len,
                    gap,
                    rule_owned,
                    &nodes,
                    &inherited_owned,
                    &opts_owned,
                    cuts,
                    used,
                    width,
                    rows,
                )
            },
        );
    Some(
        ColumnFlow::new(
            build,
            SharedString::from(plain),
            count,
            col_w,
            gap,
            measure_font(inherited, opts),
            size,
            line,
            // `column-fill: auto` с заданной высотой: колонки заполняются
            // подряд до неё (css-multicol-1 §3.3).
            match (
                e.style.used().column_fill_auto,
                dimensions.preferred[1],
                dimensions.maximum[1],
            ) {
                (Some(true), Some(Len::Px(h)), _) if h > 0.0 => Some(h),
                // Высота авто, но задан `max-height`: колонки заполняются подряд до
                // него (css-multicol-1 §column-fill `auto`: «fill columns
                // sequentially»; Blink `ConstrainColumnBlockSize` —
                // `ResolveInitialMaxBlockLength(…LogicalMaxHeight())`), а не делятся
                // поровну (`columnfill-auto-max-height-001/002`: «Abcd efgh ijkl mno.»
                // — все четыре строки в первой колонке). `inherited` — `merged`
                // хоста: `max-height` уже в точках. В рекурсии это стиль ребёнка, и
                // `column_fill_auto` там пуст — ветка молчит.
                (Some(true), None, Some(Len::Px(m))) if m > 0.0 => Some(m),
                _ => None,
            },
            whole,
        )
        .in_outer(outer)
        // `orphans`/`widows` наследуются (css-break-3 §4.4), начальное — 2.
        .line_breaks(
            inherited.orphans.unwrap_or(2) as usize,
            inherited.widows.unwrap_or(2) as usize,
        )
        .hooked(),
    )
}
