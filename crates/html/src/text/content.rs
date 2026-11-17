//! Raw subtree text borrows the retained IFC buffer when its flow contains only text.

use crate::document::boxes::inline::InlineContent;
use crate::dom::{Node, text_content::gather_text};
use crate::layout::cx::LayoutCx;
use std::ops::Deref;

pub(crate) enum Content {
    Retained(InlineContent),
    Gathered(String),
}

impl Deref for Content {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Self::Retained(content) => content.text(),
            Self::Gathered(text) => text,
        }
    }
}

pub(crate) fn raw(nodes: &[Node], cx: &LayoutCx) -> Content {
    if let Some(content) = cx.doc().boxes.borrow().raw_text(nodes) {
        return Content::Retained(content);
    }
    // Atomic/block descendants own separate flows; the nearest IFC is not
    // their full DOM text. Synthetic adapter slices also take the canonical path.
    let mut text = String::new();
    gather_text(nodes, &mut text);
    Content::Gathered(text)
}
