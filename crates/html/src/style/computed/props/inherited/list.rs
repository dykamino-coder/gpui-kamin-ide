//! Маркер списка и кавычки: `list-style-type` (css-lists-3 §3.4,
//! css-counter-styles-3 §symbols-function), `list-style-position` (§3.5),
//! `list-style-image` (§3.3), `quotes` (css-content-3 §4.1).

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, ImageSource, collapse_segment_breaks, unescape_content};
use crate::style::css::LonghandId;

/// Вид маркера: стиль счётчика (имя или `symbols()`), строка или `none`.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkerType {
    Counter(String),
    Text(String),
    None,
}

/// `list-style-type: <counter-style> | <string> | none`.
pub(crate) fn marker_type(v: &str) -> Option<MarkerType> {
    if let Some(q) = v.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        let len = crate::style::css::skip_string(&v[1..], q);
        let quoted = &v[1..1 + len];
        // Строка — одна и целиком.
        if !quoted.ends_with(q) || 1 + len != v.len() {
            return None;
        }
        let text = unescape_content(&quoted[..len - 1]);
        return Some(MarkerType::Text(collapse_segment_breaks(&text)));
    }
    let decoded = crate::style::css::unescape(v);
    let keyword = decoded.to_ascii_lowercase();
    if keyword == "none" {
        return Some(MarkerType::None);
    }
    if keyword.starts_with("symbols(") {
        // Негодный анонимный стиль делает объявление негодным.
        return crate::style::generated::counter_style_rules::valid_symbols_fn(v)
            .then(|| MarkerType::Counter(decoded));
    }
    // `<counter-style-name>` — `<custom-ident>` без `none` (css-counter-styles-3
    // §2): общие слова и `default` не годятся.
    let reserved = matches!(
        keyword.as_str(),
        "inherit" | "initial" | "unset" | "revert" | "revert-layer" | "default"
    );
    (crate::style::css::selector_tokens::ident(v) && !reserved)
        .then_some(MarkerType::Counter(decoded))
}

/// `list-style-position: inside | outside`: `true` — внутри.
pub(crate) fn position(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "inside" => Some(true),
        "outside" => Some(false),
        _ => None,
    }
}

/// `list-style-image: <image> | none`.
pub(crate) fn image(v: &str, vcx: ValueCx<'_>) -> Option<Option<ImageSource>> {
    crate::style::computed::props::background::values::image_layer(v, vcx)
}

/// Кавычки: `auto` — пустой список пар (своя система кавычек языка
/// перекрывает унаследованную), `none` — без кавычек, `match-parent` — как
/// у родителя (`None`).
pub(crate) type Quotes = Option<Option<Vec<(String, String)>>>;

/// `quotes: auto | none | match-parent | [<string> <string>]+`.
pub(crate) fn quotes(v: &str) -> Option<Quotes> {
    match v.to_ascii_lowercase().as_str() {
        "none" => return Some(Some(None)),
        "auto" => return Some(Some(Some(vec![]))),
        "match-parent" => return Some(None),
        _ => {}
    }
    let mut strs = vec![];
    let mut at = 0usize;
    while at < v.len() {
        let ch = v[at..].chars().next()?;
        if ch.is_whitespace() {
            at += ch.len_utf8();
            continue;
        }
        if ch != '"' && ch != '\'' {
            return None;
        }
        let body = at + 1;
        let len = crate::style::css::skip_string(&v[body..], ch);
        if !v[body..body + len].ends_with(ch) {
            return None;
        }
        strs.push(unescape_content(&v[body..body + len - 1]));
        at = body + len;
    }
    (!strs.is_empty() && strs.len() % 2 == 0).then(|| {
        Some(Some(
            strs.chunks(2)
                .map(|p| (p[0].clone(), p[1].clone()))
                .collect(),
        ))
    })
}

pub(super) fn accepts(id: LonghandId, v: &str, vcx: ValueCx<'_>) -> bool {
    match id {
        LonghandId::ListStyleType => marker_type(v).is_some(),
        LonghandId::ListStylePosition => position(v).is_some(),
        LonghandId::ListStyleImage => image(v, vcx).is_some(),
        LonghandId::Quotes => quotes(v).is_some(),
        _ => false,
    }
}

impl Computed {
    pub(super) fn apply_list(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        match id {
            LonghandId::ListStyleType => {
                if let Some(kind) = marker_type(v) {
                    self.list_style_type = Some(kind);
                }
            }
            LonghandId::ListStylePosition => {
                if let Some(inside) = position(v) {
                    self.list_style_inside = Some(inside);
                }
            }
            LonghandId::ListStyleImage => {
                if let Some(img) = image(v, vcx) {
                    self.list_style_image = Some(img);
                }
            }
            LonghandId::Quotes => {
                if let Some(q) = quotes(v) {
                    self.quotes = q;
                }
            }
            _ => {}
        }
    }

    pub(super) fn inherit_list(&mut self, id: LonghandId) {
        match id {
            LonghandId::ListStyleType => {
                self.list_style_type = None;
            }
            LonghandId::ListStylePosition => self.list_style_inside = None,
            LonghandId::ListStyleImage => self.list_style_image = None,
            LonghandId::Quotes => self.quotes = None,
            _ => {}
        }
    }
}
