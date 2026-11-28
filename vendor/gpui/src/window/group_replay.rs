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
use crate::ScrollNodeId;

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

    /// Why `retained_replayable_with_groups` refuses, for the `why` journal:
    /// the first condition that fails, or `None` when it would replay.
    pub fn explain_replayable_with_groups(&self, prepaint: &RetainedPrepaint, paint: &RetainedPaint) -> Option<String> {
        let groups = paint.groups();
        if !groups.is_empty() && self.replay_source().is_some() {
            return Some(format!("groups {groups:?} recorded while replaying a scene layer"));
        }
        let made = self.rendered_frame.scene.groups.len();
        if groups.end > made {
            return Some(format!("groups {groups:?} past the rendered frame's {made}"));
        }
        let mut ungrouped = paint.clone();
        ungrouped.groups = (0, 0);
        (!self.retained_replayable(prepaint, &ungrouped)).then(|| "commands are not replayable".into())
    }

    /// Whether a cached view whose last paint made `groups` of the rendered
    /// frame's list can replay it (`reuse_view_paint`): its groups are copied
    /// from the rendered frame, so the commands must come from there too.
    pub(crate) fn view_groups_replayable(&self, groups: &Range<usize>) -> bool {
        groups.is_empty()
            || (self.replay_source().is_none()
                && self.layer_recording.is_none()
                && groups.end <= self.rendered_frame.scene.groups.len())
    }

    /// Replay a cached view's paint `range` with the composition `groups` it
    /// made (`view_groups_replayable`): the commands alone would composite
    /// labels whose groups this frame never made (a masked, filtered or
    /// blended box vanishes, or takes another group's pixels). Returns the
    /// range the copied groups take in this frame's list.
    pub(crate) fn reuse_view_paint(&mut self, range: Range<PaintIndex>, groups: Range<usize>) -> Range<usize> {
        let copied = self.replay_groups(groups, None);
        self.reuse_paint(range);
        self.next_frame.scene.relabel = 0;
        copied
    }

    /// Copy the `groups` of the rendered frame's list (made by a record or by
    /// a deferred draw) to the end of this frame's list, scrolled by
    /// `scrolled` (the plan's node shifts and clips in scene pixels, with each
    /// group's label node), and renumber the labels the replay copies next
    /// (`Scene::relabel`, reset by the caller). Returns the range the copies take.
    pub(super) fn replay_groups(
        &mut self,
        groups: Range<usize>,
        scrolled: Option<(&[crate::scene::GroupLabel], &dyn Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>))>,
    ) -> Range<usize> {
        let base = self.next_frame.scene.groups.len();
        let prev = &self.rendered_frame.scene;
        if groups.is_empty() || prev.groups.len() < groups.end {
            return base..base;
        }
        let delta = base as i32 - groups.start as i32;
        let still = |_: ScrollNodeId| (Point::default(), Bounds::default());
        // KaminIDE patch: the copied scroll nodes painted in a group's scene
        // land in the scene of its copy, as those of the record's own scene
        // land in the frame's (`replay_scenes_scrolled`, `land_scroll_paint`).
        let spaces: Vec<_> = prev.groups[groups.clone()].iter().map(|group| group.scene.scroll.space).collect();
        let landing = std::mem::take(&mut self.next_frame.scroll_paint.landing);
        let (mine, others): (Vec<_>, Vec<_>) = landing.into_iter().partition(|entry| spaces.contains(&entry.2));
        self.next_frame.scroll_paint.landing = others;
        let prev = &self.rendered_frame.scene;
        let mut copies = Vec::with_capacity(spaces.len());
        let mut landed = Vec::new();
        for (at, group) in prev.groups[groups].iter().enumerate() {
            let space = group.scene.scroll.space;
            let ranges = mine.iter().filter(|entry| entry.2 == space).flat_map(|entry| [entry.1.start, entry.1.end]);
            let mut edges: Vec<(usize, usize)> = ranges.map(|old| (old, 0)).collect();
            edges.sort_unstable();
            edges.dedup();
            let copy = match scrolled {
                Some((labels, node)) => {
                    Scene::replayed_group_landing(group, delta, labels.get(at).and_then(|label| label.node()), &node, &mut edges)
                }
                None => Scene::replayed_group_landing(group, delta, None, &still, &mut edges),
            };
            let land = |old: usize| edges[edges.partition_point(|edge| edge.0 < old)].1;
            let here = copy.scene.scroll.space;
            for (index, paint, _, main) in mine.iter().filter(|entry| entry.2 == space) {
                let (start, end) = (land(paint.start), land(paint.end));
                landed.push((*index, (start != usize::MAX && end != usize::MAX).then_some(start..end), here, *main));
            }
            copies.push(copy);
        }
        for (index, range, here, main) in landed {
            let node = &mut self.next_frame.scroll_nodes[index as usize];
            match range {
                Some(range) => node.land(range, here, main),
                None => node.tear(),
            }
        }
        let scene = &mut self.next_frame.scene;
        scene.groups.extend(copies);
        scene.relabel = delta;
        base..scene.groups.len()
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

    /// The draw sequence of each group of the rendered frame (`painted_sequence`).
    #[cfg(any(test, feature = "test-support"))]
    pub fn painted_group_sequences(&self) -> Vec<Vec<(String, Bounds<ScaledPixels>)>> {
        let groups = self.rendered_frame.scene.groups.iter();
        groups
            .map(|group| {
                let mut out = Vec::new();
                crate::scene::draw_sequence(&group.scene, &mut out);
                out
            })
            .collect()
    }
}
