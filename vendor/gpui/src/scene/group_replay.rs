//! KaminIDE patch: composition groups carried by a replayed command range.
//!
//! A group paints into a scene of its own (`Window::paint_group`), kept in
//! `Scene::groups` outside the command list; the list holds only its label, a
//! surface naming the group by `group` (index + 1). A replay of a range that
//! made groups copies them to the end of this scene's list and renumbers
//! every label it copies (in the range and in the copied groups, dormant ones
//! and those a reservoir holds) by the distance between the two list
//! positions (`Scene::relabel`). A scrolled replay moves a group with the node
//! its label is painted under, the group's own commands with their nodes:
//! as WebRender's picture cache keeps a picture with the primitives of its
//! tiles and cc moves an effect node's layer with its scroll node, the
//! composited surface moves with what it composites instead of the frame
//! being built again.

use super::scroll_marks::{Piece, Run};
use super::*;
use crate::ScrollNodeId;

fn shifted(group: u32, delta: i32) -> u32 {
    (group as i64 + delta as i64).max(1) as u32
}

impl Scene {
    /// `operation` with its labels renumbered by `self.relabel`.
    pub(super) fn relabeled(&self, operation: &PaintOperation) -> PaintOperation {
        self.relabel_owned(operation.clone())
    }

    /// `relabeled` of an owned command (`replay_image_operation`).
    pub(super) fn relabel_owned(&self, mut operation: PaintOperation) -> PaintOperation {
        let delta = self.relabel;
        if delta == 0 {
            return operation;
        }
        match &mut operation {
            PaintOperation::Primitive(Primitive::Surface(surface)) if surface.group > 0 => {
                surface.group = shifted(surface.group, delta);
            }
            // A dormant command is shared with the frame it came from.
            PaintOperation::Dormant(primitive) => {
                if let Primitive::Surface(surface) = &**primitive
                    && surface.group > 0
                {
                    let mut surface = surface.clone();
                    surface.group = shifted(surface.group, delta);
                    *primitive = std::rc::Rc::new(Primitive::Surface(surface));
                }
            }
            // So is a reservoir's run: one holding labels is copied once.
            PaintOperation::Reservoir(reservoir) => {
                if let Some(run) = reservoir.run.relabeled(delta) {
                    reservoir.run = std::rc::Rc::new(run);
                }
            }
            _ => {}
        }
        operation
    }

    /// The scroll node each group of `groups` (an index range of this
    /// scene's list) has its label painted under, if any: labels in `ops` of
    /// this scene and in the groups' own scenes (nested groups).
    pub(crate) fn group_label_nodes(&self, ops: Range<usize>, groups: Range<usize>) -> Vec<Option<ScrollNodeId>> {
        let mut nodes = vec![None; groups.len()];
        if groups.is_empty() || ops.end > self.paint_operations.len() {
            return nodes;
        }
        let first = groups.start;
        let mut note = |group: u32, node: ScrollNodeId| {
            if let Some(slot) = (group as usize).checked_sub(first + 1).and_then(|at| nodes.get_mut(at)) {
                *slot = Some(node);
            }
        };
        self.label_nodes(ops, &mut note);
        for group in self.groups.get(groups).unwrap_or_default() {
            group.scene.label_nodes(0..group.scene.paint_operations.len(), &mut note);
        }
        nodes
    }

    fn label_nodes(&self, ops: Range<usize>, note: &mut impl FnMut(u32, ScrollNodeId)) {
        for mark in self.scroll.within(&ops) {
            match &self.paint_operations[mark.op as usize] {
                PaintOperation::Primitive(Primitive::Surface(surface)) => note(surface.group, mark.node),
                PaintOperation::Dormant(primitive) => {
                    if let Primitive::Surface(surface) = &**primitive {
                        note(surface.group, mark.node);
                    }
                }
                _ => {}
            }
        }
        for operation in &self.paint_operations[ops] {
            if let PaintOperation::Reservoir(reservoir) = operation {
                for item in &reservoir.run.items {
                    if let Piece::Primitive(Primitive::Surface(surface)) = &item.piece {
                        note(surface.group, reservoir.run.node);
                    }
                }
            }
        }
    }

    /// Whether the groups `groups` of this finished scene can be copied by a
    /// scrolled replay whose plan knows the nodes `known` accepts: a group
    /// whose label is under a node (`nodes`, `group_label_nodes`) holds only
    /// commands such a replay moves (`scrollable`) under known nodes; one
    /// whose label is under none holds no command of a node.
    pub(crate) fn groups_scrollable(
        &self,
        groups: Range<usize>,
        nodes: &[Option<ScrollNodeId>],
        known: impl Fn(ScrollNodeId) -> bool,
    ) -> bool {
        let Some(groups) = self.groups.get(groups) else { return false };
        groups.iter().zip(nodes).all(|(group, node)| {
            let scene = &group.scene;
            let all = 0..scene.paint_operations.len();
            let reservoirs = scene.paint_operations.iter().filter_map(|operation| match operation {
                PaintOperation::Reservoir(reservoir) => Some(reservoir.run.node),
                _ => None,
            });
            match node {
                Some(_) => {
                    scene.scroll.marks.iter().all(|mark| known(mark.node))
                        && reservoirs.into_iter().all(&known)
                        && scene.scrollable(&all, std::iter::once(all.clone()))
                }
                None => scene.scroll.marks.is_empty() && reservoirs.count() == 0,
            }
        })
    }

    /// A copy of `group` for a replay renumbering labels by `delta`: its
    /// commands replayed (scrolled with the plan's `node` shifts and clips
    /// when its label lies under `label`), its bounds moved with the label.
    pub(crate) fn replayed_group(
        group: &PaintGroup,
        delta: i32,
        label: Option<ScrollNodeId>,
        node: &impl Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>),
    ) -> PaintGroup {
        Self::replayed_group_landing(group, delta, label, node, &mut [])
    }

    /// `replayed_group` that also tells where the commands at `edges` (old
    /// indices in the group's scene, sorted) land in the copy's scene, as
    /// `replay_scrolled` tells it: the scroll nodes painted in the group take
    /// their ranges in the copy (`Window::replay_groups`). A copy that is
    /// not one command per command (an unscrolled replay that dropped one)
    /// marks them `usize::MAX`.
    pub(crate) fn replayed_group_landing(
        group: &PaintGroup,
        delta: i32,
        label: Option<ScrollNodeId>,
        node: &impl Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>),
        edges: &mut [(usize, usize)],
    ) -> PaintGroup {
        let same = |edges: &mut [(usize, usize)], exact: bool| {
            for edge in edges.iter_mut() {
                edge.1 = if exact { edge.0 } else { usize::MAX };
            }
        };
        // Unmoved and with no labels to renumber, the replay would be an
        // exact copy: share the content instead (one copy across frames).
        let labels = || group.scene.surfaces.iter().any(|surface| surface.group > 0);
        if label.is_none() && (delta == 0 || !labels()) {
            same(edges, true);
            return group.clone();
        }
        let mut scene = Scene { in_group: true, relabel: delta, ..Scene::default() };
        let all = 0..group.scene.paint_operations.len();
        let shift = match label {
            Some(id) => {
                scene.replay_scrolled(all, &group.scene, node, edges);
                node(id).0
            }
            None => {
                scene.replay(all.clone(), &group.scene);
                same(edges, scene.paint_operations.len() == all.len());
                Point::default()
            }
        };
        scene.relabel = 0;
        scene.finish();
        Scene::copy_group(group, scene, shift)
    }
}

impl Run {
    /// This run with its labels renumbered by `delta`; `None` without labels.
    fn relabeled(&self, delta: i32) -> Option<Run> {
        let labels = |item: &super::scroll_marks::Item| {
            matches!(&item.piece, Piece::Primitive(Primitive::Surface(surface)) if surface.group > 0)
        };
        if !self.items.iter().any(labels) {
            return None;
        }
        let mut run = self.clone();
        for item in &mut run.items {
            if let Piece::Primitive(Primitive::Surface(surface)) = &mut item.piece
                && surface.group > 0
            {
                surface.group = shifted(surface.group, delta);
            }
        }
        Some(run)
    }
}

#[cfg(test)]
#[path = "group_replay_tests.rs"]
mod tests;
