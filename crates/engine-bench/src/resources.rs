//! Подготовка локального входа хостом; её стоимость отделена от HTML→DOM движка.

use super::stylesheet;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, Node, NodeData, RcDom, SerializableHandle};
use std::{cell::RefCell, path::Path, rc::Rc};
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
    let bytes = std::fs::read(path)?;
    let html = kamin_html::encoding::sniff(&bytes)
        .decode(&bytes)
        .0
        .into_owned();
    // Хосту нужен DOM только для ссылок; движок получает исходную строку,
    // если замены не нужны. Этот разбор не выдаётся за фазу parse движка.
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(html.clone());
    let mut base = Url::from_file_path(path).map_err(|_| "invalid HTML file URL")?;
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
                text.parent.set(Some(Rc::downgrade(&style)));
                style.children.borrow_mut().push(text);
                let parent = node
                    .parent
                    .take()
                    .and_then(|p| p.upgrade())
                    .ok_or("orphan link")?;
                style.parent.set(Some(Rc::downgrade(&parent)));
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
mod tests {
    use super::*;

    #[test]
    fn linked_css_keeps_order_and_its_own_resource_base() {
        let dir =
            std::env::temp_dir().join(format!("kamin-bench-resources-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("css")).unwrap();
        std::fs::write(dir.join("css/asset.png"), b"fixture").unwrap();
        std::fs::write(dir.join("css/import.css"), "p { color: green }").unwrap();
        std::fs::write(
            dir.join("css/main.css"),
            "@import 'import.css'; p {background:url(asset.png)}",
        )
        .unwrap();
        let path = dir.join("input.html");
        std::fs::write(&path, "<!doctype html><meta charset=utf-8><link rel=stylesheet href=css/main.css><style>p{color:blue}</style><p>x").unwrap();
        let html = read(&path).unwrap();
        assert!(html.contains("color: green"));
        assert!(html.find("color: green") < html.find("color:blue"));
        assert!(
            html.contains(
                Url::from_file_path(dir.join("css/asset.png"))
                    .unwrap()
                    .as_str()
            )
        );
        for file in [
            "css/asset.png",
            "css/import.css",
            "css/main.css",
            "input.html",
        ] {
            std::fs::remove_file(dir.join(file)).unwrap();
        }
        std::fs::remove_dir(dir.join("css")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
