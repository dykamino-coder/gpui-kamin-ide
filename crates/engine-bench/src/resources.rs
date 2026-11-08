//! Подготовка локального входа хостом; её стоимость отделена от HTML→DOM движка.

use super::stylesheet;
use html5ever::{
    tendril::TendrilSink,
    tree_builder::{NodeOrText, TreeSink},
};
use markup5ever_rcdom::{Handle, Node, NodeData, RcDom, SerializableHandle};
use std::{borrow::Cow, cell::RefCell, io::Read, path::Path, rc::Rc};
use url::Url;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn attr(node: &Handle, key: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &node.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|a| &*a.name.local == key)
        .map(|a| a.value.to_string())
}

pub(super) fn read(path: &Path) -> Result<String> {
    Ok(read_into(path, &mut Vec::new())?.into_owned())
}

/// Читает вход в `source` одним выделением по размеру файла и отдаёт разметку для
/// движка. UTF-8 без замен заимствуется из `source` без копии; буфер живёт
/// только до постройки документа, а не весь замер.
pub(super) fn read_into<'a>(path: &Path, source: &'a mut Vec<u8>) -> Result<Cow<'a, str>> {
    source.clear();
    std::fs::File::open(path)?.read_to_end(source)?;
    let source: &'a [u8] = source;
    let html = kamin_html::encoding::sniff(source).decode(source).0;
    let base = Url::from_file_path(path).map_err(|_| "invalid HTML file URL")?;
    if !host_tokens(&html) {
        return Ok(html);
    }
    Ok(Cow::Owned(rewrite(base, html.into_owned())?))
}

/// Есть ли в разметке то, на что хост действует: теги, которые он встраивает,
/// разрешает или отвергает, и CSS `url(`/`@import`, которые раскрывает
/// `stylesheet::expand` (он ищет их буквально). Элемент возникает только из
/// начального тега (`<image>` дерево строит как `img`), а ссылки символов
/// раскрываются в тексте `<style>` только внутри SVG/MathML — их хост тоже
/// отдаёт полному разбору. Без этих токенов полный разбор вернул бы вход как есть.
fn host_tokens(html: &str) -> bool {
    const TAGS: [&[u8]; 9] = [
        b"link", b"base", b"img", b"image", b"iframe", b"object", b"embed", b"svg", b"math",
    ];
    let bytes = html.as_bytes();
    let at = |i: usize, word: &[u8]| {
        bytes
            .get(i..i + word.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(word))
    };
    bytes.iter().enumerate().any(|(i, &b)| match b {
        b'<' => TAGS.iter().any(|tag| at(i + 1, tag)),
        b'(' => i >= 3 && at(i - 3, b"url"),
        b'@' => at(i + 1, b"import"),
        _ => false,
    })
}

/// Полный разбор хоста: встраивает связанный CSS и раскрывает ресурсы.
fn rewrite(mut base: Url, html: String) -> Result<String> {
    // Хосту нужен DOM только для ссылок; движок получает исходную строку,
    // если замены не нужны. Этот разбор не выдаётся за фазу parse движка.
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(html.clone());
    let mut nodes = vec![dom.document.clone()];
    let mut ordered = Vec::new();
    while let Some(node) = nodes.pop() {
        nodes.extend(node.children.borrow().iter().rev().cloned());
        ordered.push(node);
    }
    if let Some(href) = ordered.iter().find_map(|node| match &node.data {
        NodeData::Element { name, .. } if &*name.local == "base" => attr(node, "href"),
        _ => None,
    }) {
        base = base.join(&href)?;
    }
    let mut changed = false;
    for node in ordered {
        let NodeData::Element { name, .. } = &node.data else {
            continue;
        };
        match name.local.as_ref() {
            "link"
                if attr(&node, "rel").is_some_and(|rel| {
                    let words: Vec<_> = rel.split_ascii_whitespace().collect();
                    words.iter().any(|w| w.eq_ignore_ascii_case("stylesheet"))
                        && !words.iter().any(|w| w.eq_ignore_ascii_case("alternate"))
                }) && attr(&node, "disabled").is_none() =>
            {
                let href = attr(&node, "href").ok_or("stylesheet link has no href")?;
                let mut css = stylesheet::load(&base.join(&href)?, &mut vec![])?;
                if let Some(media) = attr(&node, "media").filter(|s| !s.trim().is_empty()) {
                    css = format!("@media {media} {{\n{css}\n}}");
                }
                if css.to_ascii_lowercase().contains("</style") {
                    return Err(
                        "linked CSS contains an HTML style end tag; cannot inline faithfully"
                            .into(),
                    );
                }
                let mut style_name = name.clone();
                style_name.local = "style".into();
                let style = Node::new(NodeData::Element {
                    name: style_name,
                    attrs: Default::default(),
                    template_contents: Default::default(),
                    mathml_annotation_xml_integration_point: false,
                });
                let text = Node::new(NodeData::Text {
                    contents: RefCell::new(css.into()),
                });
                dom.append(&style, NodeOrText::AppendNode(text));
                let parent_handle = node.parent.take().ok_or("orphan link")?;
                let parent = parent_handle.upgrade().ok_or("orphan link")?;
                style.parent.set(Some(parent_handle));
                for child in parent.children.borrow_mut().iter_mut() {
                    if Rc::ptr_eq(child, &node) {
                        *child = style.clone();
                    }
                }
                changed = true;
            }
            "style" => {
                for child in node.children.borrow().iter() {
                    if let NodeData::Text { contents } = &child.data {
                        let old = contents.borrow().to_string();
                        let new = stylesheet::expand(&old, &base, &mut vec![])?;
                        if old != new {
                            *contents.borrow_mut() = new.into();
                            changed = true;
                        }
                    }
                }
            }
            "img" => {
                if let Some(src) = attr(&node, "src").filter(|src| !src.starts_with("data:")) {
                    stylesheet::file(&base.join(&src)?)?;
                }
            }
            "iframe" | "object" | "embed" => {
                return Err(
                    "nested documents are outside the single-document benchmark scope".into(),
                );
            }
            _ => {}
        }
    }
    if !changed {
        return Ok(html);
    }
    let mut bytes = Vec::new();
    html5ever::serialize(
        &mut bytes,
        &SerializableHandle::from(dom.document),
        Default::default(),
    )?;
    Ok(String::from_utf8(bytes)?)
}

#[cfg(test)]
mod tests;
