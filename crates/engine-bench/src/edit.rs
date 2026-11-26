//! Deterministic markup edits for the benchmark edit mode.
//! Attribute and class edits use DOM identity; text and leaf edits use source positions.

mod class;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Edit {
    /// Одна буква текстового узла после середины документа.
    Text,
    /// Toggle `data-edit` on the middle eligible element in DOM preorder.
    Attr,
    /// Класс `alt` у корневого `html` (так переключают стиль пробы браузеров).
    Class,
    /// Новый узел: копия первого листового элемента после середины, сразу за ним.
    Insert,
    /// Удаление первого листового элемента после середины документа.
    Remove,
}

impl Edit {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "text" => Self::Text,
            "attr" => Self::Attr,
            "class" => Self::Class,
            "insert" => Self::Insert,
            "remove" => Self::Remove,
            _ => return Err("edit must be text, attr, class, insert or remove".into()),
        })
    }

    /// Разметка второго варианта.
    pub(crate) fn markup(self, html: &str) -> Result<String> {
        let edited = match self {
            Self::Text => text(html),
            Self::Attr => attr(html),
            Self::Class => class::toggle(html),
            Self::Insert => leaf(html).map(|leaf| {
                let mut out = html.to_owned();
                out.insert_str(leaf.end, &html[leaf.start..leaf.end]);
                out
            }),
            Self::Remove => leaf(html).map(|leaf| {
                let mut out = html.to_owned();
                out.replace_range(leaf, "");
                out
            }),
        };
        edited.ok_or_else(|| format!("no place for a {self:?} edit in the input").into())
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

/// Change an attribute on a parsed element, preserving HTML tag and tree identity.
fn attr(html: &str) -> Option<String> {
    use html5ever::tendril::TendrilSink;
    use markup5ever_rcdom::{NodeData, RcDom, SerializableHandle};

    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(html);
    let mut pending = vec![dom.document.clone()];
    let mut elements = Vec::new();
    while let Some(node) = pending.pop() {
        if let NodeData::Element { name, .. } = &node.data {
            if matches!(
                &*name.local,
                "head" | "script" | "style" | "template" | "title"
            ) {
                continue;
            }
            if !matches!(&*name.local, "html" | "body") {
                elements.push(node.clone());
            }
        }
        pending.extend(node.children.borrow().iter().rev().cloned());
    }
    let target = elements.get(elements.len() / 2)?;
    let NodeData::Element { attrs, .. } = &target.data else {
        return None;
    };
    let mut attrs = attrs.borrow_mut();
    if let Some(attribute) = attrs
        .iter_mut()
        .find(|a| a.name.ns.is_empty() && &*a.name.local == "data-edit")
    {
        attribute.value = if &*attribute.value == "1" { "2" } else { "1" }.into();
    } else {
        attrs.push(html5ever::Attribute {
            name: html5ever::QualName::new(None, Default::default(), "data-edit".into()),
            value: "1".into(),
        });
    }
    drop(attrs);
    let mut bytes = Vec::new();
    html5ever::serialize(
        &mut bytes,
        &SerializableHandle::from(dom.document),
        Default::default(),
    )
    .ok()?;
    String::from_utf8(bytes).ok()
}

/// Первый листовой элемент после середины: `<имя ...>текст</имя>` без вложенных
/// тегов. Копия такого элемента рядом с ним допустима в любом родителе (ячейка
/// таблицы, пункт списка, строчный элемент), поэтому вставка и удаление меняют
/// ровно один узел дерева, а не то, как разбирается окружение.
fn leaf(html: &str) -> Option<std::ops::Range<usize>> {
    let bytes = html.as_bytes();
    let mut at = middle(html);
    while let Some(offset) = html[at..].find('<') {
        let start = at + offset;
        let open = start + 1;
        at = open;
        if !bytes.get(open).is_some_and(u8::is_ascii_alphabetic) || in_raw_text(html, open) {
            continue;
        }
        let name_end = (open..html.len())
            .find(|&i| !bytes[i].is_ascii_alphanumeric())
            .unwrap_or(html.len());
        let name = &html[open..name_end];
        if ["script", "style", "template", "title"]
            .iter()
            .any(|raw| name.eq_ignore_ascii_case(raw))
        {
            continue;
        }
        let Some(tag_end) = html[name_end..].find('>').map(|e| name_end + e) else {
            return None;
        };
        if bytes[tag_end - 1] == b'/' {
            continue;
        }
        let content = tag_end + 1;
        let Some(next) = html[content..].find('<').map(|e| content + e) else {
            return None;
        };
        let close = &html[next..];
        let closes = close.len() > name.len() + 2
            && close.starts_with("</")
            && close[2..2 + name.len()].eq_ignore_ascii_case(name)
            && close.as_bytes()[2 + name.len()] == b'>';
        if closes {
            return Some(start..next + name.len() + 3);
        }
    }
    None
}

#[cfg(test)]
mod tests;
