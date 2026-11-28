//! KaminIDE patch: scene layers drawn by a copy of their commands.
//!
//! A layer whose content made groups (filters, opacity, masks, blend modes)
//! keeps them in its scene with layer-local ids, as a frame keeps its own:
//! nested groups first, each a shared handle (`PaintGroup::scene`). The
//! renderer draws groups only from the frame's list, so a frame draws such a
//! layer by copying its top commands relabelled past the frame's groups and
//! appending the layer's groups as handles: no group content is copied, and
//! the ranges the host recorded inside the layer keep indexing the layer
//! (box runs, `rerecord_layer_box`). A frame after a write to the layer
//! copies it again from the layer (`paint_scene_layer`), never an old copy.
//!
//! A layer whose groups name the frame's scroll nodes or 3D planes, or that
//! hoisted backdrops (`SceneLayer::framed`), is drawn the same way and the
//! copy also takes what needs the frame: its backdrops paint first, its
//! scroll marks follow its commands, and the scroll nodes painted inside the
//! layer take the copy's ranges, so the frame scrolls it as its own paint.
//! Such a layer is still the one target of value writes and box runs: an
//! edit inside it repaints only the written runs, and the next frame copies
//! the layer again, as Blink's `PaintController` copies cached subsequences
//! around the repainted display items instead of painting the chunk anew.

use super::*;
use crate::scene::{BoxRange, LayerHandle, PaintOperation, SceneLayer};

impl SceneLayer {
    /// Whether a frame draws the layer by a copy of its commands rather than
    /// by one command: it kept composition groups or hoisted backdrops.
    pub fn copied(&self) -> bool {
        !self.scene().groups.is_empty() || !self.scene().hoisted_backdrops.is_empty()
    }

    /// Whether the copy needs the frame's scene: hoisted backdrops, a plane
    /// of a 3D context, or scroll marks or reservoirs with groups, which name
    /// the frame's scroll nodes by the indices they were painted at. A frame
    /// scrolls such a copy as its own paint (`Window::paint_grouped_layer`),
    /// so a scrolled replay of the frame moves the copy, not the layer.
    pub fn framed(&self) -> bool {
        let scene = self.scene();
        let unscrolled = |scene: &Scene| {
            scene.scroll.marks.is_empty()
                && !scene.paint_operations.iter().any(|op| matches!(op, PaintOperation::Reservoir(_)))
        };
        !scene.hoisted_backdrops.is_empty()
            || (!scene.groups.is_empty()
                && !(unscrolled(scene)
                    && scene.groups.iter().all(|group| {
                        group.projection.is_none() && group.fragments.is_empty() && unscrolled(&group.scene)
                    })))
    }
}

impl Window {
    /// `replay_retained_paint` of a record that drew `layer` and nothing else
    /// in the scene (the document's layer, `record_scene_layer`): when the
    /// layer is copied into frames, its listeners and states replay from the
    /// rendered frame and its commands are copied from the layer as it is
    /// now, which box runs may have been written into since that frame. The
    /// scroll nodes the record's prepaint copied land on the new copy when it
    /// has the rendered copy's commands one for one (`land_scroll_paint`).
    pub fn replay_retained_paint_with_layer(&mut self, record: &RetainedPaint, layer: &LayerHandle) -> RetainedPaint {
        let content = layer.content();
        if !content.copied() || self.replay_source().is_some() {
            return self.replay_retained_paint(record);
        }
        debug_assert!(record.replayable_with_groups(), "non-replayable paint record");
        let (start, base) = (self.paint_index(), self.next_frame.scene.groups.len());
        let range = record.range();
        self.reuse_paint_state(&range);
        let landed = self.next_frame.scene.paint_operations.len();
        self.paint_grouped_layer(layer, &content, false);
        if content.framed() {
            let space = self.rendered_frame.scene.scroll.space;
            self.land_scroll_paint(range.start.scene_index..range.end.scene_index, landed, space);
        }
        self.allow_replayed(record);
        let made = base..self.next_frame.scene.groups.len();
        RetainedPaint::of(start..self.paint_index(), true, record.allowed).grouped(made)
    }

    /// Hand the box runs painted in group `index` (nested groups' runs
    /// naming their local ids, now moved by `base`) to the scene around:
    /// its own group's runs, else the layer being recorded.
    pub(super) fn adopt_group_runs(&mut self, runs: Option<Vec<BoxRange>>, index: u32, base: u32) {
        let Some(mut runs) = runs else { return };
        for run in &mut runs {
            run.group = Some(run.group.map_or(index, |nested| nested + base));
        }
        let book = &mut self.next_frame.scene.book;
        if let Some(outer) = book.group_runs.as_mut() {
            outer.extend(runs);
        } else if let Some(record) = book.record_runs.as_mut() {
            // A paint record's copy carries them (`placed_groups`).
            record.boxes.extend(runs);
        } else if let Some(recording) = self.layer_recording.as_mut() {
            recording.boxes.extend(runs);
        }
    }

    /// Draw a layer copied into frames (`SceneLayer::copied`): its backdrops
    /// paint first, its groups join the frame's list, its top commands are
    /// copied with their labels renumbered and their scroll marks. `fresh`:
    /// the layer was just recorded (all of it is new paint), and the scroll
    /// nodes painted inside a framed layer take the copy's ranges, or are
    /// torn when the copy is not one command per command; else the frame
    /// differs from its last copy only where the layer was written since
    /// (the cell's damage), and the caller lands the nodes the replay copied.
    pub(super) fn paint_grouped_layer(&mut self, layer: &LayerHandle, content: &SceneLayer, fresh: bool) {
        let frame = &mut self.next_frame;
        let written = layer.0.damage.take();
        if fresh {
            frame.scene.book.fresh += 1;
        } else if let Some(rect) = written {
            frame.scene.book.layer_damage = Some(frame.scene.book.layer_damage.map_or(rect, |d| d.union(&rect)));
        }
        let source = content.scene();
        if !source.hoisted_backdrops.is_empty() {
            let backdrops = source.hoisted_backdrops.iter().map(|surface| (surface.clone(), None)).collect();
            frame.scene.adopt_backdrops(backdrops);
        }
        let base = frame.scene.groups.len() as i32;
        let still = |_: crate::ScrollNodeId| (Point::default(), Bounds::default());
        let groups = source.groups.iter().map(|group| Scene::replayed_group(group, base, None, &still));
        frame.scene.groups.extend(groups.collect::<Vec<_>>());
        let landed = frame.scene.paint_operations.len();
        frame.scene.scroll.raise = 0;
        frame.scene.relabel = base;
        frame.scene.replay_record(source);
        frame.scene.relabel = 0;
        let count = source.paint_operations.len();
        frame.scene.copy_scroll_marks(source, 0..count, landed);
        if !fresh || !content.framed() {
            return;
        }
        let exact = frame.scene.paint_operations.len() - landed == count;
        let (from, here) = (source.scroll.space, frame.scene.scroll.space);
        let map = |range: &Range<usize>| landed + range.start..landed + range.end;
        for node in frame.scroll_nodes.iter_mut().filter(|node| node.painted_in == Some(from)) {
            if exact {
                node.paint = node.paint.as_ref().map(map);
                node.more = node.more.iter().map(map).collect();
                node.painted_in = Some(here);
            } else {
                node.tear();
            }
        }
        for dependent in frame.scroll_dependents.iter_mut().filter(|dependent| exact && dependent.2 == from) {
            (dependent.1, dependent.2) = (landed + dependent.1, here);
        }
    }
}
