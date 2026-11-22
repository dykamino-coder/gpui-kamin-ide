//! Toggle the browser probe's class token on the parsed document element.

use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom, SerializableHandle};

pub(super) fn toggle(html: &str) -> Option<String> {
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(html);
    let root = dom
        .document
        .children
        .borrow()
        .iter()
        .find(|node| matches!(&node.data, NodeData::Element { name, .. } if &*name.local == "html"))
        .cloned()?;
    let NodeData::Element { attrs, .. } = &root.data else {
        return None;
    };
    let mut attrs = attrs.borrow_mut();
    if let Some(attribute) = attrs
        .iter_mut()
        .find(|a| a.name.ns.is_empty() && &*a.name.local == "class")
    {
        // DOMTokenList uses an ordered set and HTML's five ASCII whitespace characters.
        let mut tokens = Vec::new();
        for token in attribute
            .value
            .split(['\t', '\n', '\x0C', '\r', ' '])
            .filter(|token| !token.is_empty())
        {
            if !tokens.contains(&token) {
                tokens.push(token);
            }
        }
        if tokens.contains(&"alt") {
            tokens.retain(|token| *token != "alt");
        } else {
            tokens.push("alt");
        }
        attribute.value = tokens.join(" ").into();
    } else {
        attrs.push(html5ever::Attribute {
            name: html5ever::QualName::new(None, Default::default(), "class".into()),
            value: "alt".into(),
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
