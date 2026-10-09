//! Generated content resolves attributes using the originating element language.
//! CSS 2.1 section 12.2 and CSS Values 5 #attr-notation use host-language matching.

use std::cell::Cell;

thread_local! {
    static XHTML: Cell<bool> = const { Cell::new(false) };
}

/// The parser's existing XHTML adapter recognizes declarations and root xmlns.
pub(super) fn is_xhtml(html: &str) -> bool {
    let mut cut = html.len().min(2048);
    while !html.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &html[..cut];
    head.trim_start().starts_with("<?xml") || head.contains("http://www.w3.org/1999/xhtml")
}

pub(super) struct Document(bool);

/// Keep nested parsing from changing the originating document's case rules.
pub(super) fn document(html: &str) -> Document {
    Document(XHTML.with(|xml| xml.replace(is_xhtml(html))))
}

impl Drop for Document {
    fn drop(&mut self) {
        XHTML.with(|xml| xml.set(self.0));
    }
}

pub(super) fn html_attributes(namespace: &str) -> bool {
    namespace == "http://www.w3.org/1999/xhtml" && !XHTML.with(Cell::get)
}

/// Текст из составляющих `content` (css-content-3 §2): строки как есть,
/// счётчики — знаками своего стиля, `attr()` — значением атрибута хозяина.
///
/// Общий для `::before`/`::after` и для `::marker { content }`: по
/// css-lists-3 §content-property содержимое маркера строится «exactly as for
/// ::before».
pub(crate) fn content_text(
    items: &[crate::computed::ContentItem],
    counters: &mut crate::counters::Counters,
    attrs: &[(String, String)],
    own_quotes: Option<&Option<Vec<(String, String)>>>,
    html_attrs: bool,
) -> String {
    let mut text = String::new();
    for item in items {
        match item {
            crate::computed::ContentItem::Quote { open, emit } => {
                text.push_str(&counters.quote(*open, *emit, own_quotes));
            }
            crate::computed::ContentItem::Str(sv) => text.push_str(sv),
            crate::computed::ContentItem::Image(_) => {}
            crate::computed::ContentItem::Counter(name, style_name) => {
                let value = counters.value_of(name);
                text.push_str(&crate::counter_style::repr(value, style_name));
            }
            crate::computed::ContentItem::Counters(name, sep, style_name) => {
                // Вся цепочка области — от внешнего счётчика к внутреннему,
                // склеенная разделителем (css-lists-3 §counters).
                let chain: Vec<String> = counters
                    .chain_of(name)
                    .into_iter()
                    .map(|v| crate::counter_style::repr(v, style_name))
                    .collect();
                text.push_str(&chain.join(sep));
            }
            crate::computed::ContentItem::Attr(name) => {
                if let Some((_, v)) = attrs
                    .iter()
                    .find(|(k, _)| k == name || (html_attrs && k.eq_ignore_ascii_case(name)))
                {
                    text.push_str(v);
                }
            }
        }
    }
    text
}
