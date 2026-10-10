//! Поток колонок.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::probe::size_monolith;
use crate::paint::effects::paint_scope::DepthScope;
use crate::paint::effects::paint_scope::snapshot as defer_depth;
use crate::render::{RenderOpts, blocks, is_blank, measure_font, split_nodes};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-mctextflow-2026-09.md`, пакет A,
// 3 хунка): рекурсия `column_flow` переносит `column-gap`/`column-fill`,
// зазор и высоту заливки из `inherited`. Срез 659 пар многоколоночников,
// база тем же списком: **+6 / −4**, и все четыре потери его —
// `out-of-flow-in-multicolumn-052` 0.00 -> 99.00 (обвал в «красное
// видно»), `block-max-height-004` 0.00 -> 1.23, `multicol-gap-negative-
// 001` 0.00 -> 0.91, `multicol-height-001` 0.48 -> 0.64. Поверх пакета B
// он стоит +7/−4 при чистых +6/−1 у одного B. Возвращать только с
// разбором того, почему перенос зазора ломает внепоточные и
// отрицательный зазор.
pub(crate) fn column_flow(
    e: &Element,
    inherited: &Computed,
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
    let stretch = match inherited.height.or(e.style.height) {
        Some(Len::Px(h)) if h > 0.0 => {
            if e.style.border_box == Some(true) {
                let b = e.style.borders();
                (|| {
                    Some(
                        h - px(&e.style.padding.top)?
                            - px(&e.style.padding.bottom)?
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
fn column_flow_in(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    whole: bool,
    stretch: Option<f32>,
) -> Option<AnyElement> {
    let all_inline = e.children.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(child) => child.inline && child.style.display.is_none(),
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
        _ => size * normal_fraction(inherited, opts),
    };
    // Промежуток — ВЫЧИСЛЕННОЕ значение (css-values-4 §5): `em` в точки переводит
    // `Computed::resolve_em`, а он живёт в наследовании, то есть в `inherited`
    // (`merged` хоста у вызовов из `element()`), но НЕ в `e.style`. Пока читалось
    // `e.style`, `column-gap: 10em` подменялся кеглем (`multicol-gap-000`: вторая
    // колонка на 310 вместо 400; `-gap-fraction-001`, `-gap-large-002`). Стопка
    // читает `merged` с пакета B (`used_gap`), текстовый путь — нет.
    // ★ Это узкая часть откаченного пакета A (`scout-mctextflow-2026-09.md`): ни
    // переноса через рекурсию (там `inherited` — стиль РЕБЁНКА, его промежуток
    // пуст, значение прежнее), ни высоты. Потеря `multicol-gap-negative-001` того
    // отката — `column-gap: -1em` доезжал `Px(-20)`: промежуток `[0,∞]`
    // (css-align-3 §column-row-gap), отрицательный невалиден — умолчание, как было.
    let gap = match inherited.column_gap.or(e.style.column_gap) {
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
        if e.style.column_rule_visible == Some(true) {
            Some((
                rule_px(&e.style.column_rule_width, size),
                e.style.column_rule_color.or(inherited.color).unwrap_or(
                    crate::style::values::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                ),
            ))
        } else {
            None
        };
    let nodes = e.children.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let depth = defer_depth();
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
                depth,
                cuts,
                used,
                width,
            )
        });
    Some(
        crate::layout::float::split_flow::ColumnFlow::new(
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
                e.style.column_fill_auto,
                inherited.height.or(e.style.height),
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
        .into_any_element(),
    )
}

// `<br>` — жёсткий разрыв: в собранном тексте он помечается U+2028,
// замер режет по нему принудительно. В сырых узлах <br> текста не несёт,
// поэтому маркер в счёт сырых байт не входит.
fn gather_cols(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) if e.tag == "br" => out.push('\u{2028}'),
            Node::Element(e) => gather_cols(&e.children, out),
        }
    }
}

// Схлопываемые пробелы — только пробел, таб и переводы строк (css-text-3
// §4.1.1 «document white space characters»); `str::trim` снимал и U+00A0:
// поток из одних `&nbsp;` целиком считался пустым и шёл мимо колонок
// (`multicol-rule-color-inherit-001`).
fn css_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

#[allow(clippy::too_many_arguments)]
fn non_inline_columns(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    whole: bool,
    stretch: Option<f32>,
    all_inline: bool,
) -> Option<Option<AnyElement>> {
    if !all_inline {
        // Один-единственный блок с текстом — это тот же поток, только в своей
        // коробке: колонки режут его строки, а не обходят стороной. Разметка
        // теста колонок почти всегда такая (`<div class=multicol><div>…`).
        let mut blocks = e.children.iter().filter(|n| !is_blank(n));
        let (Some(Node::Element(only)), None) = (blocks.next(), blocks.next()) else {
            return Some(None);
        };
        if only.style.position.is_some() || only.style.float.is_some_and(|f| f != 0) {
            return Some(None);
        }
        // Вложенный многоколоночник с заданной высотой — не «тот же поток»: его
        // строки идут СВОИМИ колонками, рядами во внешних (`nested_rows_box`,
        // `flow::OUTER_ROW`), а текстовый путь разложил бы их по внешним
        // колонкам (`multicol-breaking-000…006`).
        if nested_rows_box(only) {
            return Some(None);
        }
        let inside = inherit(inherited, &only.style);
        return Some(column_flow_in(
            only,
            &inside,
            opts,
            count,
            col_w,
            whole || size_monolith(only),
            stretch,
        ));
    }
    None
}

fn normalized_text(raw_plain: &str) -> (String, Vec<(usize, usize)>) {
    let (normalized, norm_to_raw): (String, Vec<(usize, usize)>) = {
        let mut out = String::new();
        let mut map = Vec::new();
        let mut nodes_off = 0usize;
        let mut prev_space = false;
        for ch in raw_plain.chars() {
            let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
            if ch == '\u{2028}' {
                if prev_space && out.ends_with(' ') {
                    out.pop();
                    map.pop();
                }
                map.push((out.len(), nodes_off));
                out.push('\n');
                prev_space = true;
                continue;
            }
            if is_space {
                if !prev_space {
                    map.push((out.len(), nodes_off));
                    out.push(' ');
                }
            } else {
                map.push((out.len(), nodes_off));
                out.push(ch);
            }
            prev_space = is_space;
            nodes_off += ch.len_utf8();
        }
        (out, map)
    };
    (normalized, norm_to_raw)
}

#[allow(clippy::too_many_arguments)]
fn build_text_columns(
    stretch: Option<f32>,
    norm_to_raw: &[(usize, usize)],
    lead: usize,
    raw_len: usize,
    gap: f32,
    rule_owned: Option<(f32, crate::style::values::value::Color)>,
    nodes: &[Node],
    inherited_owned: &Computed,
    opts_owned: &RenderOpts,
    depth: crate::paint::effects::paint_scope::Depth,
    cuts: &[usize],
    used: usize,
    width: gpui::Pixels,
) -> AnyElement {
    let _depth = DepthScope::enter(depth);
    // Куски текста по местам разрезов: каждый — своя колонка.
    let mut parts: Vec<Vec<Node>> = vec![];
    let mut rest = nodes.to_vec();
    let mut base = 0usize;
    for cut in cuts {
        let full = cut + lead;
        let raw_cut = match norm_to_raw.binary_search_by_key(&full, |p| p.0) {
            Ok(i) => norm_to_raw[i].1,
            Err(i) => norm_to_raw.get(i).map(|p| p.1).unwrap_or(raw_len),
        };
        let (head, tail) = split_nodes(&rest, raw_cut.saturating_sub(base));
        parts.push(head);
        base = raw_cut;
        rest = tail;
    }
    parts.push(rest);
    // Разрез по жёсткому разрыву оставляет сам <br> в начале хвоста
    // (нулевая длина ставит его после разреза) — новая колонка
    // начиналась бы с пустой строки.
    for part in parts.iter_mut().skip(1) {
        while let Some(first) = part.first() {
            match first {
                Node::Element(e) if e.tag == "br" => {
                    part.remove(0);
                }
                Node::Text(t) if t.trim_matches(css_ws).is_empty() => {
                    part.remove(0);
                }
                _ => break,
            }
        }
    }
    // Ширина колонки и линейки — от used count: контента может быть
    // меньше, чем колонок (rule-001: две колонки, две строки).
    let n_cols = used.max(cuts.len() + 1);
    // §3.4 (11): `max(0, …)` — при промежутках шире коробки колонка нулевой
    // ширины, а не отрицательной (`multicol-gap-large-001`: 4 × 80 в 220).
    // Переполняющие колонки (`column-fill: auto`, разрезов больше, чем
    // колонок) ширину не делят: они той же ширины за краем коробки
    // (css-multicol-1 §8.2).
    let w_cols = if used > 0 { used } else { n_cols };
    let inner = ((f32::from(width) - gap * (w_cols - 1) as f32) / w_cols as f32).max(0.0);
    // Линейка между колонками (`column-rule`, css-multicol §4):
    // абсолютный держатель по центру промежутка на всю высоту ряда —
    // линейка шире промежутка накрывает соседние колонки (rule-001),
    // а при недозаполненных колонках всё равно тянется на всю их
    // высоту (rule-004). Рисуется ДО колонок: под контентом.
    let rule = rule_owned.filter(|(w, _)| *w > 0.0);
    let mut row = div().flex().flex_row().w(width).gap_x(px(gap)).relative();
    if let Some((rw, color)) = rule {
        if let Some(h) = stretch {
            row = row.min_h(px(h));
        }
        // css-multicol-1 §column-gaps-and-rules: «Column rules are only
        // drawn between two columns that both have content» (Blink
        // `PaintColumnRules` рисует между соседними column box, а их заводит
        // только под содержимое). Прежде линейка стояла у каждой колонки
        // used count: `multicol-count-computed-003/005` (4 слова в 3
        // колонках, вторая линейка лишняя), `columnfill-auto-max-height-
        // 001/002` (всё в первой колонке, красная линейка у пустых).
        let filled = parts
            .iter()
            .filter(|p| p.iter().any(|n| !is_blank(n)))
            .count()
            .min(n_cols);
        for i in 0..filled.saturating_sub(1) {
            let center = inner * (i as f32 + 1.0) + gap * i as f32 + gap / 2.0;
            row = row.child(
                div()
                    .absolute()
                    .left(px(center - rw / 2.0))
                    .top_0()
                    .bottom_0()
                    .w(px(rw))
                    .bg(color.to_hsla()),
            );
        }
    }
    for part in parts.into_iter() {
        row = row.child(
            div()
                .w(px(inner))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .children(blocks(&part, inherited_owned, opts_owned)),
        );
    }
    row.into_any_element()
}
