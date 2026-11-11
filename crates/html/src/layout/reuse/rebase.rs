//! The block records over a rebuild of the same source DOM (`Store::rebase`).
//!
//! A rebuild for a batch of attribute mutations knows the elements it
//! restyled (`dom::invalidation`): only those differ in their cascaded
//! values, so only their boxes are compared, and a block without one keeps
//! its record unseen (Gecko restyles and diffs only the elements its
//! invalidation reached, `RestyleManager::ProcessPostTraversal`). Paint-only
//! values of a restyled `html`/`body` reach every block as paint-only values:
//! the records are repainted in place, without a walk of either tree.

mod keys;

use super::{Store, flow_blocks};
use crate::dom::{CascadedMemo, Element, Node, same_cascaded_box, same_cascaded_tree};
use keys::Roots;
use std::collections::HashMap;

impl Store {
    /// The document `old` was rebuilt as `new` from the same source DOM
    /// (`Document::apply`, same-shape `Document::update`): the node ids are
    /// the same. `restyled`: the elements the rebuild restyled, if known;
    /// otherwise each record's block is compared in the two trees by its
    /// cascaded values (`dom::same_cascaded_tree`: both computed from them
    /// under the root boxes, whose layout values the flow key compares). A
    /// kept record drops its source, a block of the previous frame tree,
    /// which then goes with the previous document, as for a fresh one,
    /// instead of living into the next build. A block is equal only if the
    /// `html`/`body` boxes cascade equal values too (their paint-only values
    /// reach it as its own); a block equal but for paint-only values keeps
    /// its record only if it was far from the screen (`Damage::Paint` stands
    /// only there). Replay records, which hold their output, go. After a
    /// patch since the last frame (`patch`), the records hold only by the
    /// next build's comparison.
    pub(crate) fn rebase(&self, old: &[Node], new: &[Node], restyled: Option<&[u64]>) {
        if self.patched.get() {
            return;
        }
        self.reset_replays();
        let ((was, a), (now, b)) = (flow_blocks(old), flow_blocks(new));
        let mut memo = CascadedMemo::default();
        let known = match (&a, &b, restyled) {
            (Some(a), Some(b), Some(ids)) => invalidated(was, now, a, b, ids, &mut memo),
            _ => None,
        };
        let mut records = self.records.take();
        let recorded = records.len();
        // What the root boxes' comparison tells the flow keys (`carry_keys`).
        let mut roots_seen = match (&a, &b) {
            (Some(_), Some(_)) => Roots::Differ,
            _ => Roots::Unknown,
        };
        let keep = |far: bool, exact: bool, layout: &dyn Fn() -> bool| match exact {
            true => Some(false),
            false => (far && layout()).then_some(true),
        };
        match known {
            Some((None, _)) => records.clear(),
            Some((Some(roots), blocks)) => {
                roots_seen = Roots::Equal(roots);
                records.retain(|id, record| {
                    // Most rebuilds restyle no block: no lookup per record then.
                    let pair = if blocks.is_empty() {
                        None
                    } else {
                        blocks.get(id)
                    };
                    let verdict = match pair {
                        Some(&(x, y)) => keep(
                            record.far,
                            roots && same_cascaded_tree(x, y, false, &mut memo),
                            &|| same_cascaded_tree(x, y, true, &mut CascadedMemo::default()),
                        ),
                        None => keep(record.far, roots, &|| true),
                    };
                    verdict
                        .inspect(|&repaint| {
                            record.rebased(repaint);
                            record.kept.set(false);
                        })
                        .is_some()
                })
            }
            None => {
                let roots = match (&a, &b) {
                    (Some(a), Some(b)) => {
                        a.len() == b.len()
                            && a.iter()
                                .zip(b)
                                .all(|(&x, &y)| same_cascaded_box(x, y, false, &mut memo))
                    }
                    _ => false,
                };
                if let (Some(a), Some(b)) = (&a, &b)
                    && (roots
                        || a.len() == b.len()
                            && a.iter()
                                .zip(b)
                                .all(|(&x, &y)| same_cascaded_box(x, y, true, &mut memo)))
                {
                    roots_seen = Roots::Equal(roots);
                }
                // The same source DOM: the flows have the same children in
                // the same order, so the blocks pair by position.
                for (x, y) in elements(was, now) {
                    let Some(record) = records.get(&x.node_id) else {
                        continue;
                    };
                    let exact = roots && same_cascaded_tree(x, y, false, &mut memo);
                    let repaint = !exact && record.far && same_cascaded_tree(x, y, true, &mut memo);
                    if exact || repaint {
                        record.rebased(repaint);
                    }
                }
                // A record not kept had no equal block and goes.
                records.retain(|_, r| r.kept.replace(false));
            }
        }
        // A block whose record the restyle dropped is built by the next frame;
        // recorded there it would hold its subtree only until the next such
        // restyle drops it again (Blink keeps no display items of an object
        // that needs paint until it is painted clean).
        self.rebased.set(true);
        self.carry_keys(roots_seen);
        if records.len() < recorded {
            self.restyled.set(true);
        }
        // The last frame wrapped blocks and none gave a record: the next would
        // wrap them again for nothing, so the run of restyles covers them.
        let tried = !self.unrecorded.borrow().is_empty();
        if records.is_empty() && tried {
            self.restyled.set(true);
        }
        if records.is_empty() && (recorded > 0 || self.restyled.get()) {
            // No record holds: the store starts over as a fresh document's,
            // whose flow keys do not hold the previous `html`/`body` boxes.
            // With no record made or tried yet the key is carried instead:
            // the rebuild's own frame records the blocks, so that the first
            // restyle after a load already serves the second.
            self.start_over();
            return;
        }
        self.records.replace(records);
    }
}

/// Pairs of flow blocks of the two trees by node id.
type Blocks<'a> = HashMap<u64, (&'a Element, &'a Element)>;

/// The element pairs of two flows by position.
fn elements<'a>(
    was: &'a [Node],
    now: &'a [Node],
) -> impl Iterator<Item = (&'a Element, &'a Element)> {
    was.iter().zip(now).filter_map(|pair| match pair {
        (Node::Element(a), Node::Element(b)) => Some((a, b)),
        _ => None,
    })
}

/// The comparison by the restyled elements `ids` only: whether the root
/// boxes `a`/`b` (`html`, `body`) cascade equal values (`None`: not even
/// but for paint-only values), and the pairs of flow blocks that hold a
/// restyled element, by node id. `None`: a restyled element was not placed,
/// so every block is compared.
fn invalidated<'a>(
    was: &'a [Node],
    now: &'a [Node],
    a: &[&'a Element],
    b: &[&'a Element],
    ids: &[u64],
    memo: &mut CascadedMemo<'a>,
) -> Option<(Option<bool>, Blocks<'a>)> {
    let paired = |x: &[&Element], y: &[&Element]| {
        x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.node_id == q.node_id)
    };
    if !paired(a, b) || was.len() != now.len() {
        return None;
    }
    let mut roots = true;
    let mut blocks = HashMap::new();
    let flow = b.last()?.node_id;
    for &id in ids {
        if let Some(at) = b.iter().position(|e| e.node_id == id) {
            if !same_cascaded_box(a[at], b[at], false, memo) {
                // Layout values of a root box reach every block: the flow key
                // differs, and no record holds.
                if !same_cascaded_box(a[at], b[at], true, memo) {
                    return Some((None, blocks));
                }
                roots = false;
            }
            continue;
        }
        // Ids count elements as the walk leaves them: a descendant of the
        // flow has a smaller one than the flow, an element after it larger.
        if id > flow {
            continue;
        }
        let at = holder(now, id)?;
        match (&was[at], &now[at]) {
            // A block that differs in more than paint-only values may change
            // what follows it in document order (CSS counters and list numbers
            // baked into later boxes): then every block is compared.
            (Node::Element(x), Node::Element(y))
                if x.node_id == y.node_id && same_cascaded_tree(x, y, true, memo) =>
            {
                blocks.insert(y.node_id, (x, y));
            }
            _ => return None,
        }
    }
    Some((Some(roots), blocks))
}

/// The position of the flow child of `flow` that holds the element `id`:
/// the first with an id not smaller (ids grow as the walk leaves elements),
/// or an anonymous one before it.
fn holder(flow: &[Node], id: u64) -> Option<usize> {
    let first = flow
        .iter()
        .position(|n| block(n).is_some_and(|e| e.node_id >= id))?;
    let start = flow[..first]
        .iter()
        .rposition(|n| block(n).is_some_and(|e| e.node_id != 0))
        .map_or(0, |at| at + 1);
    (start..=first).find(|&at| block(&flow[at]).is_some_and(|e| holds(e, id)))
}

/// The element `n`, if it is one.
fn block(n: &Node) -> Option<&Element> {
    match n {
        Node::Element(e) => Some(e),
        Node::Text(_) => None,
    }
}

/// The subtree of `e` has the element `id`.
fn holds(e: &Element, id: u64) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if e.node_id == id {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            Node::Text(_) => None,
        }));
    }
    false
}
