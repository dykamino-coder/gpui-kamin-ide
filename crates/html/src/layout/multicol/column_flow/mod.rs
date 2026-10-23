//! Текстовый поток колонок: многоколоночник из сплошного строчного содержимого режется по
//! строкам самого текста (`ColumnFlow` из `float::split_flow`), а не стопкой блоков.
//! Здесь собираются входы разрезчика: нормализованный текст и карта его байтов на сырые
//! узлы, кегль и интерлиньяж, вычисленный `column-gap`, линейка колонок, высота заливки
//! для `column-fill: auto` и `orphans`/`widows`; содержимое с блоками уходит в `blocks`.

use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::values::value::Len;
use crate::text::fonts::style_font::measure_font;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, SharedString};
mod text;
use text::{build_text_columns, css_ws, gather_cols, normalized_text};
mod blocks;
use blocks::non_inline_columns;
mod element;
pub(crate) use element::ColumnFlow;
mod measure;

// Не переносить через рекурсию `column_flow` `column-gap`/`column-fill`,
// зазор и высоту заливки из `inherited`: перенос зазора ломает внепоточные
// и отрицательный зазор (`out-of-flow-in-multicolumn-052`,
// `block-max-height-004`, `multicol-gap-negative-001`,
// `multicol-height-001`). Вернуть можно только с разбором этой причины.
/// Многоколоночный поток из сплошного текста.
///
/// Годится, когда всё содержимое — строчное: тогда режется сам текст. Если
/// внутри блоки, колонки набираются из них сеткой, как и раньше.
pub(crate) fn column_flow(
    e: &Element,
    inherited: &crate::style::computed::BoxStyle,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
) -> Option<AnyElement> {
    // Линейки последней (единственной) линии колонок тянутся до низа
    // содержимого коробки заданной высоты (Blink `PaintColumnRules`: «Paint
    // column rules as tall as the entire multicol container, but only when at
    // the last row»; `multicol-rule-004`: две строки в коробке 5em — линейка
    // на все 5em). Высота содержимого — заданная `height` в точках.
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let stretch = match inherited.height.or(e.style.computed().height) {
        Some(Len::Px(h)) if h > 0.0 => {
            if e.style.computed().border_box == Some(true) {
                let b = e.style.computed().borders();
                (|| {
                    Some(
                        h - px(&e.style.computed().padding.top)?
                            - px(&e.style.computed().padding.bottom)?
                            - px(&b.top)?
                            - px(&b.bottom)?,
                    )
                })()
            } else {
                Some(h)
            }
        }
        _ => None,
    }
    .filter(|h| *h > 0.0);
    column_flow_in(e, inherited, opts, count, col_w, false, stretch)
}

/// `whole` — текст пришёл рекурсией из единственного ребёнка-МОНОЛИТА
/// (`size_monolith`; css-contain-2 §containment-size: «Size containment boxes
/// are monolithic», Blink `layout_box.cc:3564-3575` `IsMonolithic`). Строки
/// монолита между колонками не расходятся; в узкой колонке `measure_columns`
/// держит для него прежний сторож «без разрезов» (`contain-size-breaks-001`).
// The column builder closes over all metrics needed to rebuild a fragment.
#[allow(clippy::type_complexity)]
fn column_flow_in(
    e: &Element,
    inherited: &crate::style::computed::BoxStyle,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    whole: bool,
    stretch: Option<f32>,
) -> Option<AnyElement> {
    // Позиционированный строчный ребёнок: его содержащий блок — не колонка,
    // а предок (css-position-3 §containing-block; Blink кладёт OOF во
    // фрагментаинеры от СВОЕГО содержащего блока,
    // `OutOfFlowLayoutPart::LayoutFragmentainerDescendants`). Текстовый путь
    // знает только строки — такой поток идёт стопкой со статическими местами.
    if e.children.iter().any(|n| {
        matches!(n, Node::Element(c) if c.inline && matches!(
            c.style.computed().position,
            Some(crate::style::computed::Position::Absolute | crate::style::computed::Position::Fixed)
        ))
    }) {
        return None;
    }
    let all_inline = e.children.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(child) => child.inline && child.style.computed().display.is_none(),
    });
    if let Some(value) =
        non_inline_columns(e, inherited, opts, count, col_w, whole, stretch, all_inline)
    {
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
    let gap = match inherited.column_gap.or(e.style.computed().column_gap) {
        Some(Len::Px(v)) if v >= 0.0 => v,
        _ => size,
    };
    // Линейка колонок: видима при заданном стиле; цвет — currentColor.
    let rule_px = |w: &Option<Len>, size: f32| match w {
        Some(Len::Px(v)) => *v,
        Some(Len::Em(k)) => k * size,
        _ => 3.0,
    };
    let rule_owned: Option<(f32, crate::style::values::value::Color)> =
        if e.style.computed().column_rule_visible == Some(true) {
            Some((
                rule_px(&e.style.computed().column_rule_width, size),
                e.style.computed().column_rule_color.resolve(
                    inherited
                        .color
                        .unwrap_or(crate::style::values::value::Color::BLACK),
                ),
            ))
        } else {
            None
        };
    let nodes = e.children.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let build: std::rc::Rc<dyn Fn(&[usize], usize, gpui::Pixels) -> AnyElement> =
        std::rc::Rc::new(move |cuts: &[usize], used: usize, width: gpui::Pixels| {
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
            )
        });
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
                e.style.computed().column_fill_auto,
                inherited.height.or(e.style.computed().height),
                inherited.max_height,
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
        // `orphans`/`widows` наследуются (css-break-3 §4.4), начальное — 2.
        .line_breaks(
            inherited.orphans.unwrap_or(2) as usize,
            inherited.widows.unwrap_or(2) as usize,
        )
        .hooked(),
    )
}
