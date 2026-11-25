//! The passes of a full build over the boxes of one rebuilt subtree, under
//! copies of its ancestors' retained boxes (the spine): document passes that
//! read an ancestor (a grid that lifts positioned boxes, a flex container,
//! `display: inherit`, the writing mode) see the same ancestors as in the
//! document, and a pass that would move a box out of the subtree or wrap it
//! shows as a spine that is no longer one chain.

use super::super::fold::Parent;
use crate::document::revision::patch::element_at;
use crate::dom::{Children, Element, Node};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// What the passes of one subtree read beside its boxes.
pub(super) struct Passes<'a> {
    pub(super) values: &'a Rc<crate::values::DocumentValues>,
    pub(super) xhtml: bool,
    /// The values of the wrappers `unwrap_document` removed, for the boxes
    /// at the top of the tree.
    pub(super) folded: Option<&'a Parent>,
    /// The boxes the walk builds for those wrappers, outer first, without
    /// their children.
    pub(super) wrappers: &'a [Element],
    /// The document's base URL (`base_url::resolve`).
    pub(super) base: Option<&'a url::Url>,
}

/// The passes left every sibling of the root's box (`at`) as it was.
fn same_siblings(after: &[Node], before: &[Node], at: usize) -> bool {
    let mut memo = crate::dom::CascadedMemo::default();
    after.len() == before.len()
        && after
            .iter()
            .zip(before)
            .enumerate()
            .all(|(i, pair)| match pair {
                _ if i == at => true,
                (Node::Element(a), Node::Element(b)) => {
                    a.node_id == b.node_id
                        && a.tag == b.tag
                        && a.children.is_empty()
                        && crate::dom::same_cascaded_box(a, b, false, &mut memo)
                }
                (Node::Text(a), Node::Text(b)) => a == b,
                _ => false,
            })
}

/// A retained box without its children.
fn stub(e: &Element) -> Element {
    let mut stub = e.clone();
    stub.children = Children::default();
    stub
}

impl Passes<'_> {
    /// The box of `handle` built as `built` (the walk's nodes of one element)
    /// after the passes of a full build, under the ancestors of the retained
    /// box at `path` of `nodes`, with the boxes of those ancestors after the
    /// passes (without children); `None` when they change the shape of the spine.
    pub(super) fn finish(
        &self,
        nodes: &[Node],
        path: &[usize],
        handle: &Handle,
        built: Vec<Node>,
    ) -> Option<(Element, Vec<Element>)> {
        // Every list on the path: the siblings as stubs (passes that read the
        // neighbours of a box, as the last child of a chain, see them), the
        // box of the path in its place; above them the removed wrappers.
        let [mut inner] = <[Node; 1]>::try_from(built).ok()?;
        let mut stubs: Vec<Vec<Node>> = Vec::with_capacity(path.len());
        let mut top = Vec::new();
        for k in (0..path.len()).rev() {
            let siblings: &[Node] = match k {
                0 => nodes,
                _ => &element_at(nodes, &path[..k])?.children,
            };
            let level: Vec<Node> = siblings
                .iter()
                .map(|n| match n {
                    Node::Element(e) => Node::Element(stub(e)),
                    text => text.clone(),
                })
                .collect();
            let mut with = level.clone();
            *with.get_mut(path[k])? = inner;
            stubs.push(level);
            if k == 0 {
                top = with;
                break;
            }
            let mut ancestor = stub(element_at(nodes, &path[..k])?);
            ancestor.children = with.into();
            inner = Node::Element(ancestor);
        }
        stubs.reverse();
        for wrapper in self.wrappers.iter().rev() {
            let mut wrapper = wrapper.clone();
            wrapper.children = top.into();
            top = vec![Node::Element(wrapper)];
        }
        let finished =
            crate::dom::restyle_probe::finish_boxes(self.values, self.xhtml, top).ok()?;
        let vcx = self.values.values();
        let mut level = crate::document::resolve_logical(finished, vcx);
        let mut above = Vec::with_capacity(self.wrappers.len() + path.len());
        for _ in self.wrappers {
            let [Node::Element(e)] = level.as_mut_slice() else {
                return None;
            };
            let children = std::mem::take(&mut e.children).into_vec();
            above.push(stub(e));
            level = children;
        }
        let (&last, _) = path.split_last()?;
        for (k, &at) in path.iter().enumerate() {
            if !same_siblings(&level, &stubs[k], at) {
                return None;
            }
            if k + 1 < path.len() {
                let Node::Element(e) = level.get_mut(at)? else {
                    return None;
                };
                let children = std::mem::take(&mut e.children).into_vec();
                above.push(stub(e));
                level = children;
            }
        }
        let Node::Element(mut e) = level.swap_remove(last) else {
            return None;
        };
        if let Some(parent) = self.folded {
            // `unwrap_document`: the child of a removed wrapper takes its values.
            let input = e.style.used();
            let own = Box::new(e.style.cascaded().clone());
            e.style = parent.fold(handle, own, vcx).into();
            e.style.set_background_paint(input.background_paint);
            e.style.set_canvas_background(input.canvas_background);
        }
        let mut one = [Node::Element(e)];
        if let Some(base) = self.base {
            let budget = self.values.revision_budget();
            let admission = crate::budget::admission::Admission::new(budget);
            crate::document::base_url::resolve(&mut one, base, false, &admission).ok()?;
        }
        let [Node::Element(e)] = one else {
            return None;
        };
        Some((e, above))
    }
}
