//! Перенос записей коробок при размещении сцены во фрагменте.

use super::{Binding, BoxRange, MANY, Placed, PlacedMap};
use crate::{Bounds, Point, ScaledPixels, Scene, Window};

/// The runs and bindings of a layer without those of the `unheld` owners,
/// whose paint reached the layer outside their runs: the layer holds no copy
/// of such a box (`LayerHandle::holds_box` is false, a value write refuses),
/// so its edit paints the layer anew instead of writing some copies.
pub(crate) fn release_unheld(bindings: &mut Vec<Binding>, boxes: &mut Vec<BoxRange>, mut unheld: Vec<u32>) {
    if unheld.is_empty() {
        return;
    }
    unheld.sort_unstable();
    let held = |owner: u32| unheld.binary_search(&owner).is_err();
    bindings.retain(|b| held(b.owner));
    boxes.retain(|b| held(b.owner));
}

/// The commands `scene` (a recording of one run's box, with its `bindings`)
/// would have where the run was placed: placed by `placed` into a new scene.
/// `None` when they cannot be placed by a shift (the box paints anew).
pub(crate) fn place_run(
    scene: Scene,
    bindings: Vec<Binding>,
    placed: Placed,
) -> Option<(Scene, Vec<Binding>)> {
    let mut out = Scene::default();
    out.book.placed_map = Some(PlacedMap::new(Some(placed), 0));
    if !out.replay_placed(&scene, placed.offset, placed.clip) {
        return None;
    }
    let map = out.book.placed_map.take()?;
    let bindings = bindings
        .into_iter()
        .filter_map(|b| {
            let m = map.ops.get(b.op as usize)?;
            (m.op < MANY && m.quad < MANY).then_some(Binding { op: m.op, quad: m.quad, ..b })
        })
        .collect();
    Some((out, bindings))
}

impl Window {
    /// Keep the box runs of the record painting into the current scene, when
    /// a layer records.
    pub(crate) fn begin_record_runs(&mut self) {
        if self.layer_recording.is_some() {
            self.next_frame.scene.book.record_runs = Some(Box::default());
        }
    }

    /// `Scene::replay_placed` of `scene` into the current scene, carrying the
    /// record's box runs and bindings to the record being painted here or to
    /// the layer being recorded. Where they cannot be carried (the record is
    /// not placed by a shift, or lands in a group's scene, composited as one
    /// result) its owners are unheld there instead.
    pub(crate) fn replay_placed_carrying(
        &mut self,
        scene: &Scene,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> bool {
        let here = &self.next_frame.scene;
        let into = here.book.record_runs.is_some() || self.layer_recording.is_some();
        let takes = scene.book.record_runs.is_some()
            && if here.in_group { here.book.record_runs.is_some() } else { self.layer_recording.is_some() };
        if takes {
            self.next_frame.scene.book.placed_map = Some(PlacedMap::new(Some(Placed { offset, clip }), 0));
        }
        let done = self.next_frame.scene.replay_placed(scene, offset, clip);
        let map = self.next_frame.scene.book.placed_map.take();
        let Some(runs) = scene.book.record_runs.as_deref().filter(|_| into) else {
            return done;
        };
        let carried = match (done, map) {
            (true, Some(map)) => runs.carry(&map, &self.next_frame.scene),
            _ => runs.dropped(),
        };
        if let Some(own) = self.next_frame.scene.book.record_runs.as_mut() {
            own.extend(carried);
        } else if let Some(recording) = self.layer_recording.as_mut() {
            recording.boxes.extend(carried.boxes);
            recording.bindings.extend(carried.bindings);
            recording.unheld.extend(carried.unheld);
        }
        done
    }
}