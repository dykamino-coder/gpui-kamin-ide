//! Мелкие общие помощники сборки: шрифт замера, деление узлов, ключи текста, сбор текста.

use crate::render::*;

pub(crate) fn measure_font(c: &Computed, opts: &RenderOpts) -> gpui::Font {
    let mut font = opts.text.font();
    font.fallbacks = crate::computed::font_family::fallbacks(c, font.fallbacks);
    if let Some(family) = c.font_family.as_ref().filter(|f| !f.is_empty()) {
        font.family = crate::fonts::alias_stretch(family, c.font_stretch)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if c.monospace == Some(true) {
        font.family = crate::metrics::mono_family_for(c.lang.as_deref()).into();
    }
    if let Some(w) = c.font_weight {
        font.weight = gpui::FontWeight(w as f32);
    }
    if c.italic == Some(true) {
        font.style = gpui::FontStyle::Italic;
    }
    font
}

/// Разрезать список узлов по смещению в их общем тексте.
///
/// Смещение считается по тому же тексту, что уходит в переносчик, поэтому
/// элементы режутся вместе с ним: `<b>` на границе разреза становится двумя.
pub(crate) fn split_nodes(nodes: &[Node], at: usize) -> (Vec<Node>, Vec<Node>) {
    let mut before = vec![];
    let mut after = vec![];
    let mut seen = 0usize;
    for node in nodes {
        if seen >= at {
            after.push(node.clone());
            continue;
        }
        match node {
            Node::Text(t) => {
                let len = t.len();
                if seen + len <= at {
                    before.push(node.clone());
                } else {
                    // Режем по границе символа, ближайшей к месту разреза.
                    let mut cut = at - seen;
                    while cut < t.len() && !t.is_char_boundary(cut) {
                        cut += 1;
                    }
                    if cut > 0 {
                        before.push(Node::Text(t[..cut].to_string()));
                    }
                    if cut < t.len() {
                        after.push(Node::Text(t[cut..].to_string()));
                    }
                }
                seen += len;
            }
            Node::Element(e) => {
                let mut text = String::new();
                gather_text(&e.children, &mut text);
                let len = text.len();
                if seen + len <= at {
                    before.push(node.clone());
                } else {
                    let (head, tail) = split_nodes(&e.children, at - seen);
                    let mut a = e.clone();
                    a.children = head;
                    let mut b = e.clone();
                    b.children = tail;
                    before.push(Node::Element(a));
                    after.push(Node::Element(b));
                }
                seen += len;
            }
        }
    }
    (before, after)
}

/// Устойчивый номер абзаца для памяти выделения.
///
/// Абзац не элемент документа, своего номера у него нет; берём отпечаток его
/// текста — от кадра к кадру он не меняется, а разные абзацы почти всегда
/// различаются.
pub(crate) fn text_id(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Внешний отступ на обёртке: то же, что делает `apply`, но только поля.
pub(crate) fn apply_margin(d: gpui::Div, c: &Computed) -> gpui::Div {
    let mut d = d;
    for (val, side) in [
        (c.margin.top, 0u8),
        (c.margin.right, 1),
        (c.margin.bottom, 2),
        (c.margin.left, 3),
    ] {
        let Some(Len::Px(v)) = val else { continue };
        d = match side {
            0 => d.mt(px(v)),
            1 => d.mr(px(v)),
            2 => d.mb(px(v)),
            _ => d.ml(px(v)),
        };
    }
    d
}

/// Схлопывание пробелов для тени — той же формы, что и в абзаце.
pub(crate) fn normalize_for_shadow(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        // Схлопываются только четыре знака CSS: идеографический и неразрывный
        // пробелы — обычные знаки со своей шириной (см. `inline.rs`).
        let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
        if is_space {
            if !prev_space {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        prev_space = is_space;
    }
    out
}

/// Текст поддерева — нужен формам (`<textarea>`, `<option>`).
pub fn gather_text_public(nodes: &[Node], out: &mut String) {
    gather_text(nodes, out)
}

pub(crate) fn gather_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => gather_text(&e.children, out),
        }
    }
}
