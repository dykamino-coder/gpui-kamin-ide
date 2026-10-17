//! Collision-free ids for `<clipPath>` definitions synthesized while an SVG
//! subtree is serialized for the rasterizer.
//!
//! A synthesized id must not equal, and no author reference may point to, any
//! id the author wrote. The prefix is chosen per serialized document: the
//! base prefix grows until no attribute value or text in the subtree contains
//! it, so no author id or `url(#…)`/`href` reference can name a synthesized
//! definition.

use crate::dom::{Element, Node};
use std::cell::RefCell;

const BASE: &str = "kamin-clip";

thread_local! {
    /// Prefix of the document being serialized; `None` outside a scope.
    static PREFIX: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// One serialized SVG document. Nested scopes reuse the outer prefix.
pub(crate) struct ClipScope {
    owner: bool,
}

impl ClipScope {
    pub(crate) fn enter(root: &Element) -> ClipScope {
        if PREFIX.with(|p| p.borrow().is_some()) {
            return ClipScope { owner: false };
        }
        let prefix = free_prefix(root);
        PREFIX.with(|p| *p.borrow_mut() = Some(prefix));
        ClipScope { owner: true }
    }
}

impl Drop for ClipScope {
    fn drop(&mut self) {
        if self.owner {
            PREFIX.with(|p| *p.borrow_mut() = None);
        }
    }
}

/// Id of a synthesized definition with content hash `hash`.
pub(super) fn clip_id(hash: u64) -> String {
    PREFIX.with(|p| match p.borrow().as_deref() {
        Some(prefix) => format!("{prefix}-{hash:x}"),
        None => format!("{BASE}-{hash:x}"),
    })
}

fn free_prefix(root: &Element) -> String {
    let mut prefix = BASE.to_string();
    while mentions(root, &prefix) {
        prefix.push('_');
    }
    prefix
}

fn mentions(e: &Element, needle: &str) -> bool {
    e.attrs.iter().any(|(_, v)| v.contains(needle))
        || e.children.iter().any(|n| match n {
            Node::Text(t) => t.contains(needle),
            Node::Element(c) => mentions(c, needle),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(attrs: Vec<(&str, &str)>, children: Vec<Node>) -> Element {
        Element {
            marks: Default::default(),
            list_item: None,
            node_id: 0,
            anim: None,
            tag: "g".into(),
            style: Default::default(),
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            inline: false,
        }
    }

    #[test]
    fn synthesized_ids_avoid_author_ids_and_references() {
        let id = {
            let _scope = ClipScope::enter(&el(vec![], vec![]));
            clip_id(0xab)
        };
        let author = el(
            vec![("id", id.as_str())],
            vec![Node::Element(el(
                vec![("clip-path", "url(#kamin-clip_-ab)")],
                vec![],
            ))],
        );
        let _scope = ClipScope::enter(&author);
        let fresh = clip_id(0xab);
        assert!(!fresh.starts_with("kamin-clip-") && !fresh.starts_with("kamin-clip_-"));
        assert!(!mentions(&author, &fresh));
    }

    #[test]
    fn nested_scope_keeps_the_document_prefix() {
        let root = el(vec![("id", "kamin-clip-1")], vec![]);
        let _outer = ClipScope::enter(&root);
        let outer = clip_id(1);
        {
            let _inner = ClipScope::enter(&el(vec![], vec![]));
            assert_eq!(clip_id(1), outer);
        }
        assert_eq!(clip_id(1), outer);
    }
}
