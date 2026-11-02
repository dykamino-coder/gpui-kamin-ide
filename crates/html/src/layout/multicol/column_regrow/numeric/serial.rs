//! Серийная зависимость auto-height flex-колонки: сумма высот и сдвиги соседей.

use super::lines::Lines;
use crate::layout::fragment::{
    flow::{
        FlowGeometry,
        blocks::{Laid, Pushed, STRUT_ID},
    },
    push::Growth,
    types::{Kid, StackChild},
};

mod measure;
mod project;

pub(super) struct Serial {
    nodes: Vec<Node>,
    leaves: Vec<Lines>,
}

struct Node {
    id: u64,
    bounds: (f32, f32),
    offset: f32,
    kind: Kind,
}

enum Kind {
    Leaf(usize),
    Column(Vec<usize>),
}

impl Serial {
    #[cfg(test)]
    pub(super) fn uncommitted(&self) -> bool {
        self.leaves
            .iter()
            .all(|l| l.pushes.iter().all(|p| p.growth.borrow().is_empty()))
    }

    pub(super) fn reset(&mut self) {
        for node in &mut self.nodes {
            node.offset = 0.;
        }
        for leaf in &mut self.leaves {
            leaf.reset();
        }
    }

    pub(super) fn state(&self) -> Vec<u32> {
        self.nodes
            .iter()
            .map(|n| n.offset.to_bits())
            .chain(
                self.leaves
                    .iter()
                    .flat_map(|l| l.extra.iter().map(|v| v.to_bits())),
            )
            .collect()
    }

    pub(super) fn accepts(&self, grows: &[Growth]) -> bool {
        grows.iter().all(|g| {
            g.amount.is_finite()
                && match &g.target {
                    Pushed::Box(id) => {
                        g.amount != 0.
                            && self.nodes[..self.nodes.len() - 1]
                                .iter()
                                .any(|n| n.id == *id)
                    }
                    Pushed::Line(_) => self
                        .leaves
                        .iter()
                        .any(|l| l.accepts(std::slice::from_ref(g))),
                    _ => false,
                }
        })
    }

    pub(super) fn grow(&mut self, child: &mut StackChild, base: &Kid, grows: Vec<Growth>) {
        for leaf in &mut self.leaves {
            leaf.add(&grows);
        }
        for node in &mut self.nodes {
            if let Some(amount) = grows
                .iter()
                .filter(|g| g.target == Pushed::Box(node.id))
                .map(|g| g.amount)
                .max_by(f32::total_cmp)
            {
                node.offset += amount;
            }
        }
        let r = child.regrow.as_mut().expect("measured serial model");
        let delta = self.project(&mut r.laid);
        child.h = base.h + delta;
        child.over = base.over + delta;
        let b = r.watched.blocks().expect("measured serial scope");
        let breaks = b
            .breaks(child.h, &r.laid)
            .finish(FlowGeometry::default(), child.h);
        super::model::record(child, breaks);
    }

    pub(super) fn commit(&self, child: &StackChild) -> bool {
        let r = child.regrow.as_ref().expect("measured serial model");
        r.watched.pin_flex_lines(&r.laid);
        let mut changed = false;
        for leaf in &self.leaves {
            changed |= leaf.commit();
        }
        for node in &self.nodes {
            changed |= r.watched.grow_flex_item(node.id, node.offset);
        }
        changed
    }
}
