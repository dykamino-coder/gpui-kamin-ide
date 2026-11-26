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

/// Where a group's label lies in the commands that composite it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupLabel {
    /// Under a scroll node, drawn, dormant or held by its reservoir.
    Under(ScrollNodeId),
    /// Drawn under no node, or in no command for an unknown reason.
    Unmarked,
    /// Dropped under its node (`PaintGroup::label_dropped`) or in a hidden
    /// group's scene: no replay a plan accepts composites the group.
    Hidden,
}

impl GroupLabel {
    pub(crate) fn node(&self) -> Option<ScrollNodeId> {
        if let GroupLabel::Under(node) = self { Some(*node) } else { None }
    }
}

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

    /// Where each group of `groups` (an index range of this scene's list) has its label:
    /// in `ops`, which made the groups (`Window::paint_group` labels a group next to its
    /// scene), or in the scene of a group whose own label is there (nested groups).
    pub(crate) fn group_labels(&self, ops: Range<usize>, groups: Range<usize>) -> Vec<GroupLabel> {
        let (first, mut labels) = (groups.start, vec![None; groups.len()]);
        let Some(all) = self.groups.get(groups).filter(|_| ops.end <= self.paint_operations.len()) else {
            return vec![GroupLabel::Unmarked; labels.len()];
        };
        self.note_labels(ops, first, &mut labels, false);
        let mut scanned = vec![false; labels.len()];
        loop {
            // The labels in the scene of each group whose own label was found.
            if let Some(at) = (0..labels.len()).find(|&at| !scanned[at] && labels[at].is_some()) {
                scanned[at] = true;
                let (scene, hidden) = (&all[at].scene, labels[at] == Some(GroupLabel::Hidden));
                scene.note_labels(0..scene.paint_operations.len(), first, &mut labels, hidden);
                continue;
            }
            match (0..labels.len()).find(|&at| labels[at].is_none() && all[at].label_dropped) {
                Some(at) => labels[at] = Some(GroupLabel::Hidden),
                None => break,
            }
        }
        labels.into_iter().map(|label| label.unwrap_or(GroupLabel::Unmarked)).collect()
    }

    /// Note the labels in `ops` (`hidden`: of a hidden group's scene).
    fn note_labels(&self, ops: Range<usize>, first: usize, labels: &mut [Option<GroupLabel>], hidden: bool) {
        let mut note = |group: u32, label: GroupLabel| {
            if let Some(slot) = (group as usize).checked_sub(first + 1).and_then(|at| labels.get_mut(at))
                && !matches!(slot, Some(GroupLabel::Under(_)))
            {
                *slot = Some(if hidden { GroupLabel::Hidden } else { label });
            }
        };
        let surface = |operation: &PaintOperation| match operation {
            PaintOperation::Primitive(Primitive::Surface(surface)) => Some(surface.group),
            PaintOperation::Dormant(primitive) => match &**primitive {
                Primitive::Surface(surface) => Some(surface.group),
                _ => None,
            },
            _ => None,
        };
        for mark in self.scroll.within(&ops) {
            if let Some(group) = surface(&self.paint_operations[mark.op as usize]) {
                note(group, GroupLabel::Under(mark.node));
            }
        }
        for operation in &self.paint_operations[ops] {
            if let PaintOperation::Reservoir(reservoir) = operation {
                for item in &reservoir.run.items {
                    if let Piece::Primitive(Primitive::Surface(surface)) = &item.piece {
                        note(surface.group, GroupLabel::Under(reservoir.run.node));
                    }
                }
            } else if let Some(group) = surface(operation) {
                note(group, GroupLabel::Unmarked);
            }
        }
    }
    /// Whether the groups `groups` of this finished scene can be copied by a
    /// scrolled replay whose plan knows the nodes `known` accepts: a group
    /// whose label is under a node (`labels`, `group_labels`) holds only
    /// commands such a replay moves (`scrollable`) under known nodes; one
    /// whose label is under none holds no command of a node; a hidden one
    /// is never seen, wherever the copy puts what it holds.
    pub(crate) fn groups_scrollable(
        &self,
        groups: Range<usize>,
        labels: &[GroupLabel],
        known: impl Fn(ScrollNodeId) -> bool,
    ) -> bool {
        let Some(groups) = self.groups.get(groups) else { return false };
        groups.iter().zip(labels).all(|(group, label)| {
            let scene = &group.scene;
            let all = 0..scene.paint_operations.len();
            let reservoirs = scene.paint_operations.iter().filter_map(|operation| match operation {
                PaintOperation::Reservoir(reservoir) => Some(reservoir.run.node),
                _ => None,
            });
            match label {
                GroupLabel::Hidden => true,
                GroupLabel::Under(_) => {
                    scene.scroll.marks.iter().all(|mark| known(mark.node))
                        && reservoirs.into_iter().all(&known)
                        && scene.scrollable(&all, std::iter::once(all.clone()))
                }
                GroupLabel::Unmarked => scene.scroll.marks.is_empty() && reservoirs.count() == 0,
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
