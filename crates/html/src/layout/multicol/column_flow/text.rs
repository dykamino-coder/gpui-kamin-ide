//! Текст колонок: сбор, нормализация пробелов и построение колонок по разрезам.

use crate::dom::Node;
use crate::paint::effects::paint_scope::DepthScope;
use crate::render::{RenderOpts, blocks, is_blank, split_nodes};
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

// `<br>` — жёсткий разрыв: в собранном тексте он помечается U+2028,
// замер режет по нему принудительно. В сырых узлах <br> текста не несёт,
// поэтому маркер в счёт сырых байт не входит.
pub(super) fn gather_cols(nodes: &[Node], out: &mut String) {
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
pub(super) fn css_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

pub(super) fn normalized_text(raw_plain: &str) -> (String, Vec<(usize, usize)>) {
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
pub(super) fn build_text_columns(
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
