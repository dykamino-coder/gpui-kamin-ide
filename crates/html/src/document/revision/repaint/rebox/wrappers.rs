//! The wrappers `unwrap_document` removed (`html`, `body`): the values the
//! boxes at the top of the tree take from them, and their boxes, under which
//! the passes of a full build run (`spine`).

use super::Walks;
use crate::document::Document;
use crate::document::revision::repaint::fold::{Parent, Unwrapped};
use crate::dom::restyle_probe::RestyleCache;
use crate::dom::{Element, Node};

impl Document {
    /// The values the boxes at the top of the tree take from the wrappers
    /// `unwrap_document` removed, and the boxes the walk builds for those
    /// wrappers (without children), outer first.
    pub(super) fn folded(
        &self,
        walks: &Walks<'_>,
        cache: &mut RestyleCache,
    ) -> Result<(Option<Parent>, Vec<Element>), &'static str> {
        let document = &walks.input.dom.document;
        let mut wrappers = Vec::new();
        let Some(unwrapped) = Unwrapped::add(&self.nodes, document, &mut wrappers) else {
            return Err("unwrapped");
        };
        let Some(unwrapped) = unwrapped else {
            return Ok((None, Vec::new()));
        };
        let values = &self.state.values;
        let styles = cache
            .kept_styles(walks.probe, walks.input, values, &wrappers)
            .map_err(|_| "budget")?;
        let styles = |i: usize| styles[i].as_ref().map(|p| &*p.style);
        let parent = unwrapped
            .parent(styles, self.state.values())
            .ok_or("unwrapped")?;
        // `unwrap_document` removes `html` with its only child `body`, or
        // `html` alone: the tree keeps the box of a wrapper it did not remove.
        let boxed = |tag: &str| {
            self.nodes
                .iter()
                .any(|n| matches!(n, Node::Element(e) if e.tag == tag && e.node_id != 0))
        };
        let built = cache
            .shallows(walks.probe, walks.input, values, &wrappers, walks.mutated)
            .map_err(|_| "budget")?;
        // The wrappers are not elements of a rebuilt subtree.
        values.traversal.take_rebuilt();
        let mut stubs = Vec::new();
        for (nodes, tag) in built.into_iter().zip(["html", "body"]) {
            if boxed(tag) {
                break;
            }
            let Ok([Node::Element(mut e)]) = <[Node; 1]>::try_from(nodes.ok_or("unwrapped")?)
            else {
                return Err("unwrapped");
            };
            e.children = crate::dom::Children::default();
            stubs.push(e);
        }
        Ok((Some(parent), stubs))
    }
}
