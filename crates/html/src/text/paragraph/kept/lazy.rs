//! The builder of a kept paragraph's atom elements in one frame (`super`).

use super::KeptAtoms;
use crate::dom::{Element, Node};
use gpui::AnyElement;

/// The top-level nodes of the paragraph that hold its atoms, cloned for the
/// frame: an atom itself, or an element whose subtree holds atoms (its
/// children are a shared snapshot, so the clone is shallow). One clone per
/// top-level element with atoms, none per text or per nested atom.
pub(crate) struct AtomSource {
    tops: Vec<(u32, Element)>,
}

impl AtomSource {
    /// The holders of `atoms` among `nodes`; `None` if a path leaves them.
    pub(crate) fn of(nodes: &[Node], atoms: &KeptAtoms) -> Option<Self> {
        let mut tops: Vec<(u32, Element)> = Vec::new();
        for k in 0..atoms.keys.len() {
            let i = *atoms.path_of(k).first()?;
            if tops.last().map(|(t, _)| *t) != Some(i) {
                let Node::Element(e) = nodes.get(i as usize)? else {
                    return None;
                };
                tops.push((i, e.clone()));
            }
        }
        Some(Self { tops })
    }

    /// The node of atom `k`, if its path still leads to its key.
    fn element<'a>(&'a self, atoms: &KeptAtoms, k: usize) -> Option<&'a Element> {
        let path = atoms.path_of(k);
        let (first, rest) = path.split_first()?;
        let at = self.tops.binary_search_by_key(first, |(i, _)| *i).ok()?;
        let mut e = &self.tops[at].1;
        for &i in rest {
            let Node::Element(next) = e.children.get(i as usize)? else {
                return None;
            };
            e = next;
        }
        (e.node_id == atoms.keys[k]).then_some(e)
    }
}

/// Builds the element of one atom node (LazyBuild).
pub(crate) type AtomFn = Box<dyn Fn(&Element) -> Option<AnyElement>>;

/// Builds the element of an atom of a resumed paragraph as the paragraph's
/// build did (`render::paragraph::atom_piece` under this frame's options).
pub(crate) struct LazyBuild {
    source: AtomSource,
    build: AtomFn,
}

impl LazyBuild {
    pub(crate) fn new(source: AtomSource, build: AtomFn) -> Self {
        Self { source, build }
    }

    pub(super) fn atom(&self, atoms: &KeptAtoms, k: usize) -> Option<AnyElement> {
        (self.build)(self.source.element(atoms, k)?)
    }
}
