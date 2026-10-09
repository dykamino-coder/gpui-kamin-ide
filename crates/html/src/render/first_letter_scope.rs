//! Keep first-letter selection within the container's first formatted line.

use super::{
    AnyElement, Computed, Display, Node, RenderOpts, blank_text, block_level_in_flow, out_of_flow,
    paragraph_probed,
};

pub(super) struct Scope {
    first: bool,
}

impl Scope {
    pub(super) fn new(nodes: &[Node], style: &Computed) -> Self {
        let block = nodes
            .iter()
            .find(|n| {
                in_flow(
                    n,
                    style.keep_spaces == Some(true),
                    style.preserve_newlines == Some(true),
                )
            })
            .is_some_and(|n| matches!(n, Node::Element(e) if block_level_in_flow(e)));
        // A block descendant supplies the first formatted line; its layer is
        // routed separately. A later anonymous paragraph cannot supply it.
        Self { first: !block }
    }

    pub(super) fn paragraph(
        &mut self,
        nodes: &[Node],
        style: &Computed,
        opts: &RenderOpts,
    ) -> AnyElement {
        self.flow(nodes, style, |style| paragraph_probed(nodes, style, opts))
    }

    pub(super) fn flow(
        &mut self,
        nodes: &[Node],
        style: &Computed,
        build: impl FnOnce(&Computed) -> AnyElement,
    ) -> AnyElement {
        let first = self.first;
        if nodes.iter().any(|n| {
            in_flow(
                n,
                style.keep_spaces == Some(true),
                style.preserve_newlines == Some(true),
            )
        }) {
            self.first = false;
        }
        if first || style.first_letter.is_none() {
            build(style)
        } else {
            let mut later = style.clone();
            later.first_letter = None;
            later.first_letter_own = None;
            build(&later)
        }
    }
}

fn in_flow(node: &Node, spaces: bool, newlines: bool) -> bool {
    match node {
        Node::Text(t) => {
            !blank_text(t) || (spaces && !t.is_empty()) || (newlines && t.contains('\n'))
        }
        Node::Element(e)
            if out_of_flow(&e.style)
                || e.style.display == Some(Display::None)
                || e.style.first_letter_excluded
                || e.tag == "::marker" =>
        {
            false
        }
        Node::Element(e) if e.style.display == Some(Display::Contents) => {
            e.children.iter().any(|n| {
                in_flow(
                    n,
                    e.style.keep_spaces.unwrap_or(spaces),
                    e.style.preserve_newlines.unwrap_or(newlines),
                )
            })
        }
        Node::Element(e) => e.tag != "wbr",
    }
}
