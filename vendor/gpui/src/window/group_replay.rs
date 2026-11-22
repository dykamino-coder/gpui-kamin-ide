//! KaminIDE patch: the composition groups of a retained paint record
//! (`scene::group_replay`).
//!
//! A record made at the frame's root keeps the index range of the groups it
//! made in the frame's list. A replay from the rendered frame copies those
//! groups to the end of the new frame's list and renumbers the labels it
//! copies by the distance, so a record with opacity, filters, masks or blend
//! modes replays instead of being built again. Only a host that replays a
//! whole frame record asks for it (`retained_replayable_with_groups`): the
//! other records keep refusing groups (`RetainedPaint::replayable`).

use super::*;
use crate::{PaintGroup, ScrollNodeId};

impl RetainedPaint {
    /// This output with the groups `made` of the frame's list.
    pub(super) fn grouped(mut self, made: Range<usize>) -> Self {
        self.groups = (made.start as u32, made.end as u32);
        self
    }

    /// The index range of the groups this output made in its frame's list.
    pub(super) fn groups(&self) -> Range<usize> {
        self.groups.0 as usize..self.groups.1 as usize
    }

    /// Whether these commands can be copied into a later frame with the
    /// composition groups they made (`Window::replay_retained_paint`,
    /// `replay_retained_paint_scrolled`).
    pub fn replayable_with_groups(&self) -> bool {
        self.replayable
    }

    /// Join the groups of `next`, painted right after this output: `false`
    /// when both made groups that are not adjacent in the list.
    pub(super) fn join_groups(&mut self, next: &RetainedPaint) -> bool {
        let (mine, theirs) = (self.groups(), next.groups());
        if theirs.is_empty() {
            return true;
        }
        if mine.is_empty() {
            self.groups = next.groups;
            return true;
        }
        self.groups.1 = next.groups.1;
        mine.end == theirs.start
    }
}

impl Window {
    /// `retained_replayable` of a record that may hold composition groups:
    /// they are copied from the rendered frame, so the record replays from
    /// it (not from a scene layer).
    pub fn retained_replayable_with_groups(&self, prepaint: &RetainedPrepaint, paint: &RetainedPaint) -> bool {
        let mut ungrouped = paint.clone();
        ungrouped.groups = (0, 0);
        (paint.groups().is_empty() || self.replay_source().is_none())
            && paint.groups().end <= self.rendered_frame.scene.groups.len()
            && self.retained_replayable(prepaint, &ungrouped)
    }

    /// Copy the groups `record` made in the rendered frame to the end of this
    /// frame's list, scrolled by `scrolled` (the plan's node shifts and clips
    /// in scene pixels, with each group's label node), and renumber the
    /// labels the replay copies next (`Scene::relabel`, reset by the caller).
    /// Returns the range the copies take.
    pub(super) fn replay_groups(
        &mut self,
        record: &RetainedPaint,
        scrolled: Option<(&[Option<ScrollNodeId>], &dyn Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>))>,
    ) -> Range<usize> {
        let groups = record.groups();
        let base = self.next_frame.scene.groups.len();
        let prev = &self.rendered_frame.scene;
        if groups.is_empty() || prev.groups.len() < groups.end {
            return base..base;
        }
        let delta = base as i32 - groups.start as i32;
        let still = |_: ScrollNodeId| (Point::default(), Bounds::default());
        let copies: Vec<PaintGroup> = prev.groups[groups]
            .iter()
            .enumerate()
            .map(|(at, group)| match scrolled {
                Some((labels, node)) => Scene::replayed_group(group, delta, labels.get(at).copied().flatten(), &node),
                None => Scene::replayed_group(group, delta, None, &still),
            })
            .collect();
        let scene = &mut self.next_frame.scene;
        scene.groups.extend(copies);
        scene.relabel = delta;
        base..scene.groups.len()
    }

    /// `replay_record` of a scene layer's content that needs the frame's scene
    /// (`RecordedLayer::Spliced`: it made groups), with what its scroll nodes
    /// painted there: their marks, ranges and dependents move to the frame's
    /// indices when the copy is one command per command, or the nodes are
    /// torn (a scrolled replay of the frame then builds).
    pub(super) fn splice_scene(&mut self, scene: &Scene) {
        let frame = &mut self.next_frame;
        let landed = frame.scene.paint_operations.len();
        frame.scene.scroll.raise = 0;
        frame.scene.replay_record(scene);
        let count = scene.paint_operations.len();
        frame.scene.copy_scroll_marks(scene, 0..count, landed);
        let exact = frame.scene.paint_operations.len() - landed == count;
        let (from, here) = (scene.scroll.space, frame.scene.scroll.space);
        let map = |range: &Range<usize>| landed + range.start..landed + range.end;
        for node in frame.scroll_nodes.iter_mut().filter(|node| node.painted_in == Some(from)) {
            if exact {
                node.paint = node.paint.as_ref().map(map);
                node.more = node.more.iter().map(map).collect();
                node.painted_in = Some(here);
            } else {
                node.torn = true;
            }
        }
        for dependent in frame.scroll_dependents.iter_mut().filter(|dependent| exact && dependent.2 == from) {
            (dependent.1, dependent.2) = (landed + dependent.1, here);
        }
    }

    /// The group labels of the rendered frame and of its groups (the group
    /// each names and the area it draws), and each group's bounds and drawn
    /// quads, in list order.
    #[cfg(any(test, feature = "test-support"))]
    pub fn painted_groups(&self) -> (Vec<(u32, Bounds<ScaledPixels>)>, Vec<(Bounds<ScaledPixels>, Vec<crate::Quad>)>) {
        let scene = &self.rendered_frame.scene;
        let scenes = std::iter::once(scene).chain(scene.groups.iter().map(|group| &*group.scene));
        let labels = scenes
            .flat_map(|scene| scene.surfaces.iter().filter(|surface| surface.group > 0))
            .map(|surface| (surface.group, surface.bounds.intersect(&surface.content_mask.bounds)))
            .collect();
        (labels, scene.groups.iter().map(|group| (group.bounds, group.scene.quads.clone())).collect())
    }
}
