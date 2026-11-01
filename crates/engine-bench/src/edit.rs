//! Локальная правка входа для режима `relayout`: вариант разметки или ширина
//! области просмотра. Правка детерминирована и не зависит от страницы, кроме
//! места: первый подходящий узел после середины разметки.

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Edit {
    /// Одна буква текстового узла после середины документа.
    Text,
    /// Новый атрибут `data-edit` у элемента после середины документа.
    Attr,
    /// Класс `alt` у корневого `html` (так переключают стиль пробы браузеров).
    Class,
    /// Ширина области просмотра на 1 px меньше (`html { width }` у пробы браузеров).
    Resize,
}

impl Edit {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "text" => Self::Text,
            "attr" => Self::Attr,
            "class" => Self::Class,
            "resize" => Self::Resize,
            _ => return Err("edit must be text, attr, class or resize".into()),
        })
    }

    /// Разметка второго варианта; `None` — правка меняет не разметку, а ширину.
    pub(crate) fn markup(self, html: &str) -> Result<Option<String>> {
        let edited = match self {
            Self::Text => text(html),
            Self::Attr => attr(html),
            Self::Class => class(html),
            Self::Resize => return Ok(None),
        };
        edited
            .map(Some)
            .ok_or_else(|| format!("no place for a {self:?} edit in the input").into())
    }
}

/// Позиция внутри `<script>`/`<style>`: правка там не меняет отрисовку.
fn in_raw_text(html: &str, at: usize) -> bool {
    let before = &html[..at];
    ["script", "style"].iter().any(|tag| {
        let open = before.rfind(&format!("<{tag}"));
        let close = before.rfind(&format!("</{tag}"));
        open.is_some_and(|open| close.is_none_or(|close| close < open))
    })
}

fn middle(html: &str) -> usize {
    let start = html.find("<body").unwrap_or(0);
    let mut at = start + (html.len() - start) / 2;
    while !html.is_char_boundary(at) {
        at += 1;
    }
    at
}

fn text(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let mut at = middle(html);
    while let Some(offset) = html[at..].find('>') {
        let open = at + offset + 1;
        let end = html[open..].find('<').map_or(html.len(), |e| open + e);
        if !in_raw_text(html, open)
            && let Some(letter) = (open..end).find(|&i| bytes[i].is_ascii_lowercase())
        {
            let replacement = if bytes[letter] == b'a' { "o" } else { "a" };
            let mut out = html.to_owned();
            out.replace_range(letter..letter + 1, replacement);
            return Some(out);
        }
        at = end;
    }
    None
}

fn attr(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let mut at = middle(html);
    while let Some(offset) = html[at..].find('<') {
        let open = at + offset + 1;
        if bytes.get(open).is_some_and(u8::is_ascii_alphabetic) && !in_raw_text(html, open) {
            let name = (open..html.len())
                .find(|&i| !bytes[i].is_ascii_alphanumeric())
                .unwrap_or(html.len());
            let mut out = html.to_owned();
            out.insert_str(name, " data-edit=\"1\"");
            return Some(out);
        }
        at = open;
    }
    None
}

fn class(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<html")? + "<html".len();
    let tag_end = open + lower[open..].find('>')?;
    if lower[open..tag_end].contains("class") {
        return None;
    }
    let mut out = html.to_owned();
    out.insert_str(open, " class=\"alt\"");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::Edit;

    fn page() -> String {
        let body = "<p>alpha</p>".repeat(10);
        format!(
            "<html><head><style>p{{a:b}}</style></head><body>{body}<script>var a</script></body></html>"
        )
    }

    #[test]
    fn text_edit_changes_one_letter_of_rendered_text_after_the_middle() {
        let page = page();
        let edited = Edit::Text.markup(&page).unwrap().unwrap();
        assert_eq!(edited.len(), page.len());
        assert_eq!(edited.matches("<p>olpha</p>").count(), 1, "{edited}");
        assert!(edited.contains("var a"));
    }

    #[test]
    fn attribute_and_class_edits_add_one_attribute() {
        let page = page();
        let edited = Edit::Attr.markup(&page).unwrap().unwrap();
        assert_eq!(
            edited.matches("<p data-edit=\"1\">alpha").count(),
            1,
            "{edited}"
        );
        let edited = Edit::Class.markup(&page).unwrap().unwrap();
        assert!(edited.starts_with("<html class=\"alt\">"), "{edited}");
        assert!(Edit::Class.markup("<html class=x>").is_err());
    }

    #[test]
    fn resize_keeps_the_markup_and_unknown_edits_are_rejected() {
        assert!(Edit::Resize.markup(&page()).unwrap().is_none());
        assert!(Edit::parse("nope").is_err());
        assert_eq!(Edit::parse("attr").unwrap(), Edit::Attr);
    }
}
