//! KaminIDE patch: scene layers that keep their composition groups.
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

use super::*;
use crate::scene::{BoxRange, LayerHandle, PaintOperation, SceneLayer};

impl Window {
    /// `replay_retained_paint` of a record that drew `layer` and nothing else
    /// in the scene (the document's layer, `record_scene_layer`): when the
    /// layer kept groups, its listeners and states replay from the rendered
    /// frame and its commands are copied from the layer as it is now, which
    /// box runs may have been written into since that frame.
    pub fn replay_retained_paint_with_layer(&mut self, record: &RetainedPaint, layer: &LayerHandle) -> RetainedPaint {
        let content = layer.content();
        if !content.has_groups() || self.replay_source().is_some() {
            return self.replay_retained_paint(record);
        }
        debug_assert!(record.replayable_with_groups(), "non-replayable paint record");
        let (start, base) = (self.paint_index(), self.next_frame.scene.groups.len());
        self.reuse_paint_state(&record.range());
        self.paint_grouped_layer(layer, &content, false);
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
        if let Some(outer) = self.next_frame.scene.book.group_runs.as_mut() {
            outer.extend(runs);
        } else if let Some(recording) = self.layer_recording.as_mut() {
            recording.boxes.extend(runs);
        }
    }

    /// Whether a layer may keep the groups `scene` made: none of them is a
    /// plane of a 3D context, and neither the layer nor a group holds scroll
    /// marks or reservoirs, which name the frame's scroll nodes by the
    /// indices they were painted at (those layers are spliced instead).
    pub(super) fn layer_keeps_groups(scene: &Scene) -> bool {
        let unscrolled = |scene: &Scene| {
            scene.scroll.marks.is_empty()
                && !scene.paint_operations.iter().any(|op| matches!(op, PaintOperation::Reservoir(_)))
        };
        unscrolled(scene)
            && scene.groups.iter().all(|group| {
                group.projection.is_none() && group.fragments.is_empty() && unscrolled(&group.scene)
            })
    }

    /// Draw a layer that kept its groups (`layer_keeps_groups`): its groups
    /// join the frame's list, its top commands are copied with their labels
    /// renumbered. `fresh`: the layer was just recorded (all of it is new
    /// paint); else the frame differs from its last copy only where the
    /// layer was written since (the cell's damage).
    pub(super) fn paint_grouped_layer(&mut self, layer: &LayerHandle, content: &SceneLayer, fresh: bool) {
        let scene = &mut self.next_frame.scene;
        let written = layer.0.damage.take();
        if fresh {
            scene.book.fresh += 1;
        } else if let Some(rect) = written {
            scene.book.layer_damage = Some(scene.book.layer_damage.map_or(rect, |d| d.union(&rect)));
        }
        let base = scene.groups.len() as i32;
        let still = |_: crate::ScrollNodeId| (Point::default(), Bounds::default());
        let groups = content.scene.groups.iter().map(|group| Scene::replayed_group(group, base, None, &still));
        scene.groups.extend(groups.collect::<Vec<_>>());
        scene.relabel = base;
        scene.replay_record(&content.scene);
        scene.relabel = 0;
    }
}
