use super::View;
use kamin_heap::retain::NONE;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Cut {
    pub matched_blocks: usize,
    pub unreachable_requested_bytes: u64,
    pub by_group: BTreeMap<String, u64>,
}

#[derive(Debug, Default, Serialize)]
pub struct Holder {
    pub edges: u64,
    pub sources: BTreeSet<u32>,
    pub offsets: BTreeMap<u32, u64>,
}

#[derive(Debug, Serialize)]
pub struct Hop {
    pub node: u32,
    pub label: String,
    /// Offset in this node's parent where its incoming pointer was observed.
    pub incoming_offset: u32,
}

impl View<'_> {
    /// Remove outgoing references of matching blocks, then count blocks no
    /// longer reachable. This is a graph experiment, not proof of deallocation.
    pub fn cut(&self, patterns: &[String]) -> Cut {
        let removed = self.matching(patterns);
        let mut seen = vec![false; self.graph.nodes()];
        seen[self.graph.root() as usize] = true;
        let mut stack = vec![self.graph.root()];
        while let Some(node) = stack.pop() {
            if removed.contains(&node) {
                continue;
            }
            for (target, _) in self.graph.edges(node) {
                if !std::mem::replace(&mut seen[target as usize], true) {
                    stack.push(target);
                }
            }
        }
        let mut by_group = BTreeMap::<String, u64>::new();
        for (node, reached) in seen[..self.graph.blocks.len()].iter().enumerate() {
            if !reached {
                *by_group
                    .entry(self.group(node as u32).to_owned())
                    .or_default() += self.graph.bytes(node as u32);
            }
        }
        Cut {
            matched_blocks: removed.len(),
            unreachable_requested_bytes: by_group.values().sum(),
            by_group,
        }
    }

    pub fn holders(&self, patterns: &[String]) -> BTreeMap<String, Holder> {
        let targets = self.matching(patterns);
        let mut holders = BTreeMap::<String, Holder>::new();
        for source in 0..self.graph.nodes() as u32 {
            if targets.contains(&source) {
                continue;
            }
            for (target, offset) in self.graph.edges(source) {
                if targets.contains(&target) {
                    let holder = holders.entry(self.label(source)).or_default();
                    holder.edges += 1;
                    holder.sources.insert(source);
                    *holder.offsets.entry(offset).or_default() += 1;
                }
            }
        }
        holders
    }

    /// Target to root, including virtual nodes. Length is bounded even for
    /// caller-provided invalid parent chains; Graph::read recalculates chains.
    pub fn path(&self, target: u32, dominators: bool) -> Vec<Hop> {
        let parent = if dominators {
            &self.graph.idom
        } else {
            &self.graph.parent
        };
        let mut node = target;
        let mut path = Vec::new();
        for _ in 0..self.graph.nodes() {
            if node == NONE || node as usize >= self.graph.nodes() {
                break;
            }
            path.push(Hop {
                node,
                label: self.label(node),
                incoming_offset: if dominators {
                    0
                } else {
                    self.graph.parent_offset[node as usize]
                },
            });
            if node == self.graph.root() {
                break;
            }
            node = parent[node as usize];
        }
        path
    }
}
