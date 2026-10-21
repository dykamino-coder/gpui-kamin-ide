//! Идентификаторы без коллизий для определений `<clipPath>`, синтезируемых при сериализации
//! SVG-поддерева для растеризатора.
//!
//! Синтезированный id не должен совпадать ни с одним id автора, и ни одна авторская ссылка не
//! должна на него указывать. Префикс выбирается один раз на сериализуемый документ и передаётся
//! вниз по сериализатору: базовый префикс растёт, пока ни одно значение атрибута или текст
//! документа его не содержит, так что ни авторский id, ни ссылка `url(#…)`/`href` не способны
//! назвать синтезированное определение.

use crate::dom::{Element, Node};

const BASE: &str = "kamin-clip";

/// Id namespace of one serialized SVG document.
pub(crate) struct ClipIds {
    prefix: String,
}

impl ClipIds {
    /// Namespace free of every string the author wrote under `root`.
    pub(crate) fn for_document(root: &Element) -> ClipIds {
        let mut prefix = BASE.to_string();
        while mentions(root, &prefix) {
            prefix.push('_');
        }
        ClipIds { prefix }
    }

    /// Id of a synthesized definition with content hash `hash`.
    pub(super) fn id(&self, hash: u64) -> String {
        format!("{}-{hash:x}", self.prefix)
    }
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
    fn plain_document_keeps_the_base_prefix() {
        let ids = ClipIds::for_document(&el(vec![("id", "a")], vec![]));
        assert_eq!(ids.id(0xab), "kamin-clip-ab");
    }

    #[test]
    fn synthesized_ids_avoid_author_ids_and_references() {
        let author = el(
            vec![("id", "kamin-clip-ab")],
            vec![Node::Element(el(
                vec![("clip-path", "url(#kamin-clip_-ab)")],
                vec![Node::Text("kamin-clip__".into())],
            ))],
        );
        let fresh = ClipIds::for_document(&author).id(0xab);
        assert_eq!(fresh, "kamin-clip___-ab");
        assert!(!mentions(&author, &fresh));
    }
}
