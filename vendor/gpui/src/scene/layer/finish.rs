//! Finish a retained layer and reduce proofs from all placements of a box.

#[cfg(debug_assertions)]
use super::ink_of_commands;
use super::{Binding, BoxRange, RecordRuns, SceneLayer, layer_counters, release_unheld};
use crate::scene::Scene;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

impl SceneLayer {
    /// Finish a recording. No absent owner may also have visible or unknown
    /// commands: checking the union precedes removal of the unheld runs.
    pub(crate) fn finish_recording(scene: Scene, mut runs: RecordRuns) -> Self {
        if !runs.omitted.is_empty() {
            let mut present = runs.unheld.clone();
            present.extend(runs.boxes.iter().map(|b| b.owner));
            present.extend(runs.bindings.iter().map(|b| b.owner));
            present.sort_unstable();
            present.dedup();
            runs.omitted
                .retain(|owner| present.binary_search(owner).is_err());
            runs.omitted.sort_unstable();
            runs.omitted.dedup();
        }
        release_unheld(&mut runs.bindings, &mut runs.boxes, runs.unheld);
        let sizes = (runs.bindings.len(), runs.boxes.len());
        let mut layer = Self::finish(scene, runs.bindings, runs.boxes);
        if layer.recorded_sizes() == sizes {
            layer.omitted = runs.omitted;
        }
        layer
    }

    /// Finish `scene`, painted with `bindings` (quad indices in paint order)
    /// and the runs of its boxes, into a layer: the scene is sorted once and
    /// the bindings follow their quads.
    pub(crate) fn finish(
        mut scene: Scene,
        mut bindings: Vec<Binding>,
        mut boxes: Vec<BoxRange>,
    ) -> Self {
        // Every binding names a quad of this scene and its command, every
        // run a range of its commands; one that does not was recorded in
        // another scene. The layer then takes no value writes and records no
        // box again (the host paints it anew), counted, never a wrong write.
        let quads = scene.quads.len();
        let operations = scene.paint_operations.len();
        let valid = bindings.iter().all(|b| {
            (b.quad as usize) < quads
                && matches!(
                    scene.paint_operations.get(b.op as usize),
                    Some(crate::scene::PaintOperation::Primitive(
                        crate::scene::Primitive::Quad(_)
                    ))
                )
        }) && boxes.iter().all(|b| b.ops.end as usize <= operations);
        if !valid {
            debug_assert!(false, "a layer binding or run names another scene");
            layer_counters::RECORDS_REFUSED.fetch_add(1, Relaxed);
            bindings.clear();
            boxes.clear();
        }
        let moved = scene.finish_tracking_quads();
        for binding in &mut bindings {
            binding.quad = moved[binding.quad as usize];
        }
        bindings.sort_by_key(|b| (b.owner, b.value, b.quad));
        // Kept while the scene was painted: no second pass over the commands.
        let bounds = scene.book.primitive_ink;
        #[cfg(debug_assertions)]
        assert!(
            bounds == ink_of_commands(&scene.paint_operations),
            "the scene's kept ink differs from its commands"
        );
        static NEXT: AtomicU64 = AtomicU64::new(1);
        layer_counters::RECORDS.fetch_add(1, Relaxed);
        let by_owner = std::cell::OnceCell::new();
        Self {
            scene,
            id: NEXT.fetch_add(1, Relaxed),
            version: 0,
            bounds,
            bindings,
            boxes,
            by_owner,
            omitted: Vec::new(),
        }
    }
}
