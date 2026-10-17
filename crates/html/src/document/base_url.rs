//! Base URLs resolve resource links once, while DOM selectors still see authored values.
use crate::dom::Node;
use markup5ever_rcdom::{Handle, NodeData};
use url::Url;

pub(super) fn document_base(html: &str, fallback: Option<&Url>) -> Option<Url> {
    fn find(node: &Handle) -> Option<String> {
        if let NodeData::Element { name, attrs, .. } = &node.data
            && &*name.local == "base"
            && &*name.ns == "http://www.w3.org/1999/xhtml"
            && let Some(attr) = attrs.borrow().iter().find(|a| &*a.name.local == "href")
        {
            return Some(attr.value.to_string());
        }
        node.children.borrow().iter().find_map(find)
    }
    let dom = crate::dom::input::html(html);
    find(&dom.document)
        .and_then(|href| {
            fallback
                .map_or_else(|| Url::parse(&href), |base| base.join(&href))
                .ok()
        })
        .or_else(|| fallback.cloned())
}

pub(super) fn resolve(nodes: &mut [Node], base: &Url, svg: bool) {
    for node in nodes {
        let Node::Element(e) = node else {
            continue;
        };
        let base = e
            .attrs
            .iter()
            .find(|(k, _)| k == "xml:base")
            .and_then(|(_, v)| base.join(v).ok())
            .unwrap_or_else(|| base.clone());
        let svg = if e.tag == "svg" {
            true
        } else if e.tag.eq_ignore_ascii_case("foreignObject") {
            false
        } else {
            svg
        };
        if svg && e.tag == "use" {
            for (name, href) in &mut e.attrs {
                if name == "href"
                    && !href.starts_with('#')
                    && let Ok(url) = base.join(href)
                {
                    *href = url.to_string();
                }
            }
        }
        resolve(&mut e.children, &base, svg);
    }
}

pub(super) fn key(html: &str, css: &str, base: Option<&Url>) -> u64 {
    use std::hash::{Hash, Hasher};
    let original = super::hash_of(html, css);
    let Some(base) = base else {
        return original;
    };
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    original.hash(&mut hash);
    base.as_str().hash(&mut hash);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hrefs(nodes: &[Node], out: &mut Vec<String>) {
        for n in nodes {
            if let Node::Element(e) = n {
                out.extend(
                    e.attrs
                        .iter()
                        .filter(|(k, _)| k == "href")
                        .map(|(_, v)| v.clone()),
                );
                hrefs(&e.children, out);
            }
        }
    }
    #[test]
    fn source_base_persists_across_update_and_changes_document_identity() {
        let html = "<svg><use href='sprite.svg#green'/></svg>";
        let mut doc = super::super::Document::new_with_base(html, "", "file:///tests/page.html");
        let mut links = Vec::new();
        hrefs(doc.nodes(), &mut links);
        assert_eq!(links, ["file:///tests/sprite.svg#green"]);
        let other = super::super::Document::new_with_base(html, "", "file:///elsewhere/page.html");
        assert_ne!(doc.salt(), other.salt());
        assert!(doc.update("<svg><use href='new.svg#blue'/></svg>", ""));
        links.clear();
        hrefs(doc.nodes(), &mut links);
        assert_eq!(links, ["file:///tests/new.svg#blue"]);
    }
    #[test]
    fn html_base_and_xml_base_resolve_without_rewriting_local_fragments() {
        let html = "<base href='assets/'><svg xml:base='icons/'><use href='sprite.svg#x'/><use href='#local'/></svg>";
        let doc = super::super::Document::new_with_base(html, "", "file:///tests/page.html");
        let mut links = Vec::new();
        hrefs(doc.nodes(), &mut links);
        assert_eq!(links, ["file:///tests/assets/icons/sprite.svg#x", "#local"]);
    }
}
