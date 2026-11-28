//! Retention views shared by DevTools and the command-line runner. All sizes
//! here describe observed blocks and conservative edges, never process commit.

use kamin_heap::retain::{Graph, NONE};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod export;
pub mod query;
pub mod wire;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Label {
    pub site: String,
    pub owner: Option<String>,
    pub subsystem: String,
}

pub struct View<'a> {
    pub graph: &'a Graph,
    pub labels: &'a [Label],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Group {
    pub objects: u64,
    pub self_bytes: u64,
    /// Dominator subtrees of this group, counting nested members only once.
    pub retained_bytes: u64,
}

impl<'a> View<'a> {
    pub fn new(graph: &'a Graph, labels: &'a [Label]) -> Result<Self, &'static str> {
        if labels.len() != graph.blocks.len() {
            return Err("graph labels have a different block count");
        }
        Ok(Self { graph, labels })
    }

    pub fn group(&self, node: u32) -> &str {
        self.labels
            .get(node as usize)
            .map_or("?", |label| label.owner.as_deref().unwrap_or(&label.site))
    }

    pub fn label(&self, node: u32) -> String {
        if node == self.graph.root() {
            return "(root)".into();
        }
        if node == self.graph.unrooted() {
            return "(unrooted: unscanned threads, OS memory or nobody)".into();
        }
        if node > self.graph.root() {
            return format!(
                "({})",
                self.graph.regions[(node - self.graph.root() - 1) as usize].name
            );
        }
        let label = &self.labels[node as usize];
        let mut text = label.owner.as_ref().map_or_else(
            || label.site.clone(),
            |owner| format!("{owner} | {}", label.site),
        );
        let flag = self.graph.flags[node as usize];
        if flag & 1 != 0 {
            text.push_str(" {unheld}");
        }
        if flag & 2 != 0 {
            text.push_str(" {cycle}");
        }
        text
    }

    pub fn matching(&self, patterns: &[String]) -> BTreeSet<u32> {
        (0..self.graph.blocks.len() as u32)
            .filter(|&node| {
                let label = self.label(node);
                patterns.iter().any(|pattern| label.contains(pattern))
            })
            .collect()
    }

    /// Dominator children, largest first with deterministic node tie-breaking.
    pub fn children(&self) -> Vec<Vec<u32>> {
        let mut children = vec![vec![]; self.graph.nodes()];
        for (node, &parent) in self.graph.idom.iter().enumerate() {
            if parent != NONE && parent as usize != node {
                children[parent as usize].push(node as u32);
            }
        }
        for kids in &mut children {
            kids.sort_by_key(|&node| (std::cmp::Reverse(self.graph.retained[node as usize]), node));
        }
        children
    }

    pub fn groups(&self) -> BTreeMap<String, Group> {
        let tree = self.children();
        let mut groups = BTreeMap::<String, Group>::new();
        let mut active = BTreeMap::<&str, usize>::new();
        let mut stack = vec![(self.graph.root(), false)];
        while let Some((node, leaving)) = stack.pop() {
            if (node as usize) < self.graph.blocks.len() {
                let name = self.group(node);
                let depth = active.entry(name).or_default();
                if leaving {
                    *depth -= 1;
                    continue;
                }
                let group = groups.entry(name.to_owned()).or_default();
                group.objects += 1;
                group.self_bytes += self.graph.bytes(node);
                if *depth == 0 {
                    group.retained_bytes += self.graph.retained[node as usize];
                }
                *depth += 1;
            } else if leaving {
                continue;
            }
            stack.push((node, true));
            stack.extend(tree[node as usize].iter().map(|&kid| (kid, false)));
        }
        groups
    }
}

/// Labels are tied to stable allocation IDs of this capture, rather than
/// addresses that may have been reused after the owner walk.
pub fn labels(
    capture: &crate::retention::Capture,
    snapshot: &crate::session::Snapshot,
    symbols: &mut crate::symbols::Symbols,
) -> Result<Vec<Label>, &'static str> {
    if capture.session != snapshot.session || capture.epoch != snapshot.epoch {
        return Err("graph and allocation snapshot belong to different epochs");
    }
    let _suppress = crate::allocation::Suppress::enter();
    let live: BTreeMap<_, _> = snapshot
        .live
        .iter()
        .map(|block| (block.id, block))
        .collect();
    let owners: BTreeMap<_, _> = snapshot
        .owners
        .iter()
        .flat_map(|owners| &owners.owners)
        .map(|owner| (owner.allocation, owner.path.as_str()))
        .collect();
    capture
        .allocation_ids
        .iter()
        .map(|id| {
            let block = live
                .get(id)
                .ok_or("graph allocation is absent from its snapshot")?;
            let names = symbols.stack(&block.stack);
            let site = kamin_heap::fold::classify::classify(names);
            Ok(Label {
                site: site.owned_type.map_or_else(
                    || site.name.clone(),
                    |owned| format!("[{owned}] {}", site.name),
                ),
                owner: owners.get(id).map(|path| (*path).to_owned()),
                subsystem: site.subsystem,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
