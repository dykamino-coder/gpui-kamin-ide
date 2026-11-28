//! The source children: an element list, or a host's child list in its own box.

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;

pub(super) enum Children<'a> {
    Elements(&'a [Element]),
    Nodes(&'a [Node], Option<(f32, Option<f32>)>),
}

impl Children<'_> {
    pub(super) fn elements(&self) -> Box<dyn Iterator<Item = &Element> + '_> {
        match self {
            Self::Elements(kids) => Box::new(kids.iter()),
            Self::Nodes(nodes, _) => Box::new(nodes.iter().filter_map(|n| match n {
                Node::Element(e) => Some(e),
                _ => None,
            })),
        }
    }
    pub(super) fn build(&self, parent: &BoxStyle, opts: &RenderOpts) -> Vec<gpui::AnyElement> {
        match self {
            Self::Elements(kids) => kids
                .iter()
                .map(|c| opts.cx.contracts.build.element(c, parent, opts))
                .collect(),
            Self::Nodes(nodes, _) => opts.cx.contracts.build.blocks(nodes, parent, opts),
        }
    }
}
