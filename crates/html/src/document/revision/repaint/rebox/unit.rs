//! The unit of one root of the box executor (`rebox`): its subtree built
//! before and after the batch, checked by the three conditions of the rule.

use super::spine;
use super::{Unit, Walks};
use crate::document::Document;
use crate::document::revision::mutation::Batch;
use crate::document::revision::patch::element_at;
use crate::document::revision::repaint::fold::Parent;
use crate::dom::restyle_probe::RestyleCache;
use crate::dom::{Element, Node, Subtree};
use markup5ever_rcdom::Handle;

impl Document {
    /// The unit of `root`, or why its parent is the root.
    pub(super) fn unit(
        &self,
        batch: &mut Batch,
        root: &Handle,
        walks: &Walks<'_>,
        cache: &mut RestyleCache,
        (folded, wrappers): (Option<&Parent>, &[Element]),
    ) -> Result<Unit, &'static str> {
        let id = walks.index.id(root).ok_or("unaddressed")?;
        let mut paths = walks.boxes.paths(id);
        let path = paths.next().ok_or("no box")?;
        if paths.next().is_some() {
            return Err("boxes");
        }
        let retained = element_at(&self.nodes, &path).ok_or("a box path")?;
        let values = &self.state.values;
        let traversal = &values.traversal;
        let walk = |cache: &mut RestyleCache| -> Result<(Subtree, Vec<usize>), &'static str> {
            let built = cache.subtrees(
                walks.probe,
                walks.input,
                values,
                std::slice::from_ref(root),
                walks.mutated,
            );
            let elements = traversal.take_rebuilt();
            let built = built.map_err(|_| "budget")?.pop().flatten();
            Ok((built.ok_or("no context")?, elements))
        };
        let old = walk(cache)?;
        batch.write();
        let new = walk(cache);
        batch.undo();
        let ((old, dropped), (new, built)) = (old, new?);
        // 3. The counters and quotes of the walk around the subtree.
        if old.outside.reads || new.outside.reads || old.outside.hands != new.outside.hands {
            return Err("counters");
        }
        let left = old.outside.left_changes(&new.outside);
        if !left.is_empty() && !followers_reset(batch, root, &left, (walks, values), cache) {
            return Err("counters");
        }
        // 2. One box of the root, the same to its parent.
        let (Some(a), Some(b)) = (own_box(&old.nodes, id), own_box(&new.nodes, id)) else {
            return Err("box");
        };
        if !same_to_parent(a, b, root) {
            return Err("box");
        }
        let base = values.images.borrow().base().cloned();
        let passes = spine::Passes {
            values,
            xhtml: walks.input.xhtml,
            folded,
            wrappers,
            base: base.as_ref(),
        };
        let (before, old_above) = passes
            .finish(&self.nodes, &path, root, old.nodes)
            .ok_or("spine")?;
        // 1. The walk of the subtree is the build's.
        let before = adopt_identities(before, retained);
        let mut memo = crate::dom::CascadedMemo::default();
        if !crate::dom::same_cascaded_tree(&before, retained, false, &mut memo) {
            return Err("isolated");
        }
        let (node, new_above) = passes
            .finish(&self.nodes, &path, root, new.nodes)
            .ok_or("spine")?;
        // The passes give the ancestors the same values with the new box.
        let mut memo = crate::dom::CascadedMemo::default();
        let same_above = old_above.len() == new_above.len()
            && old_above
                .iter()
                .zip(&new_above)
                .all(|(a, b)| crate::dom::same_cascaded_box(a, b, false, &mut memo));
        if !same_above {
            return Err("spine");
        }
        let node = adopt_identities(node, retained);
        let mut ids = Vec::new();
        // SVG definitions the document indexes by id (`masks`).
        let mut masks = false;
        for e in [retained, &node] {
            elements(e, &mut |e| {
                masks |=
                    e.tag.eq_ignore_ascii_case("mask") || e.tag.eq_ignore_ascii_case("clippath");
                if e.node_id != 0 {
                    ids.push(e.node_id);
                }
            });
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(Unit {
            handle: root.clone(),
            path,
            node,
            ids,
            dropped,
            built,
            masks,
        })
    }
}

/// The counters `names` the subtree of `root` leaves to its following
/// siblings are not read after it: the first following sibling the walk
/// enters instantiates each of them before and after the batch, which ends
/// their scope (css-lists-3 §4.4: a counter's scope is its creator, the
/// creator's following siblings and their descendants, up to a sibling that
/// instantiates the same name).
fn followers_reset(
    batch: &mut Batch,
    root: &Handle,
    names: &[&str],
    (walks, values): (&Walks<'_>, &std::rc::Rc<crate::values::DocumentValues>),
    cache: &mut RestyleCache,
) -> bool {
    use crate::style::computed::Display;
    use crate::style::generated::counters_scan::instantiates;
    let Some(up) = parent_of(root) else {
        return false;
    };
    let siblings = up.children.borrow();
    let Some(at) = siblings.iter().position(|c| std::rc::Rc::ptr_eq(c, root)) else {
        return false;
    };
    let followers: Vec<Handle> = siblings[at + 1..]
        .iter()
        .filter(|c| matches!(c.data, markup5ever_rcdom::NodeData::Element { .. }))
        .cloned()
        .collect();
    drop(siblings);
    for state in 0..2 {
        if state == 1 {
            batch.write();
        }
        let mut found = None;
        for follower in &followers {
            let one = std::slice::from_ref(follower);
            let probed = match state {
                0 => cache.kept_styles(walks.probe, walks.input, values, one),
                _ => cache.styles(walks.probe, walks.input, values, one),
            };
            let p = match probed.map(|mut v| v.pop().flatten()) {
                Ok(Some(p)) => p,
                // An element the walk does not enter takes no part in counters.
                Ok(None) => continue,
                Err(_) => break,
            };
            match p.style.reset_box_layout.display {
                Some(Display::None) => continue,
                Some(Display::Contents) => break,
                _ => {
                    found = Some(names.iter().all(|n| instantiates(&p.style, follower, n)));
                    break;
                }
            }
        }
        if state == 1 {
            batch.undo();
        }
        if found != Some(true) {
            return false;
        }
    }
    true
}

/// The walk of the parent reads the same values from the root's box `a` and
/// `b` (`walk::leave`, `walk::finish_element`): its tag and level, the column
/// role a table keeps or drops, and the ruby role a ruby parent wraps by.
/// A parent that reads the content of its children (`dir=auto`, `bdi`: the
/// first strong character, HTML §3.2.6.4) reads the whole box. What the
/// passes of the build read is the spine's check.
fn same_to_parent(a: &Element, b: &Element, root: &Handle) -> bool {
    let (x, y) = (a.style.cascaded(), b.style.cascaded());
    if x == y {
        return a.tag == b.tag && a.inline == b.inline;
    }
    // An ancestor that reads the content of its descendants.
    let mut at = parent_of(root);
    let mut reads_content = false;
    while let Some(p) = at {
        if let markup5ever_rcdom::NodeData::Element { name, attrs, .. } = &p.data {
            reads_content |= &*name.local == "bdi"
                || attrs
                    .borrow()
                    .iter()
                    .any(|a| &*a.name.local == "dir" && a.value.eq_ignore_ascii_case("auto"));
        }
        at = parent_of(&p);
    }
    !reads_content
        && a.tag == b.tag
        && a.inline == b.inline
        && x.reset_box_layout.display == y.reset_box_layout.display
        && x.reset_context.col_role == y.reset_context.col_role
        && x.reset_text_box.ruby_role == y.reset_text_box.ruby_role
}

fn parent_of(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}

/// The only node of `nodes`, the box of the element `id`.
fn own_box(nodes: &[Node], id: u64) -> Option<&Element> {
    match nodes {
        [Node::Element(e)] if e.node_id == id => Some(e),
        _ => None,
    }
}

/// `built` with the state identities of the synthetic boxes of `retained`
/// where both trees have the same shape: a rebuilt box keeps the identity
/// its frame state is kept under (`state_identity`).
fn adopt_identities(mut built: Element, retained: &Element) -> Element {
    fn walk(a: &mut Element, b: &Element) {
        if a.node_id == 0 && b.node_id == 0 && a.tag == b.tag {
            a.marks.state_id = b.marks.state_id.clone();
        }
        if a.children.len() != b.children.len() {
            return;
        }
        let pairs = a.children.iter_mut().zip(b.children.iter());
        for (x, y) in pairs {
            if let (Node::Element(x), Node::Element(y)) = (x, y) {
                walk(x, y);
            }
        }
    }
    walk(&mut built, retained);
    built
}

/// Two builds of the same nodes give the same boxes, value for value (the
/// synthetic boxes of `b` compared under the identities of `a`'s).
pub(in crate::document::revision::repaint) fn same_boxes(a: &[Node], b: &[Node]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|pair| match pair {
            (Node::Element(x), Node::Element(y)) => {
                let y = adopt_identities(y.clone(), x);
                crate::dom::same_cascaded_tree(
                    x,
                    &y,
                    false,
                    &mut crate::dom::CascadedMemo::default(),
                )
            }
            (Node::Text(x), Node::Text(y)) => x == y,
            _ => false,
        })
}

/// Every element box of the subtree of `e`, `e` first.
fn elements<'a>(e: &'a Element, visit: &mut impl FnMut(&'a Element)) {
    let mut stack = vec![e];
    while let Some(e) = stack.pop() {
        visit(e);
        for child in e.children.iter().rev() {
            if let Node::Element(c) = child {
                stack.push(c);
            }
        }
    }
}
