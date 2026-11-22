//! KaminIDE patch: the box runs of a paint record that made composition
//! groups (`window::grouped_layer`).
//!
//! Such a record is not placed by a shift and clip (`Scene::replay_placed`
//! refuses groups): its copy (`Scene::copy_record`) joins the scene with its
//! groups appended at the scene's group count (`Window::adopt_scene`). The
//! commands of a group are copied one for one, moved by the placement and
//! not clipped, so a run inside group `k` is the same range of the copy of
//! group `k`, now group `base + k`, placed by the offset alone. The record's
//! own commands are clipped by the copy: their runs cannot be placed and
//! their owners are unheld, as for any record placed as a copy.

use super::super::{Point, ScaledPixels, Scene};
use super::{Placed, RecordRuns};
use crate::window::Window;

impl RecordRuns {
    /// These runs where a copy of `source` (`copy`, moved by `offset`)
    /// appended at group `base` placed them: runs in a group follow it,
    /// the others and every binding are unheld.
    pub(crate) fn carry_grouped(&self, source: &Scene, copy: &Scene, base: u32, offset: Point<ScaledPixels>) -> RecordRuns {
        let mut out = RecordRuns { unheld: self.unheld.clone(), ..RecordRuns::default() };
        out.unheld.extend(self.bindings.iter().map(|b| b.owner));
        let placed = Placed { offset, clip: None };
        for run in &self.boxes {
            // A copy that dropped a command (no ink) moved the run's range.
            let whole = |k: u32| {
                source.groups.get(k as usize).zip(copy.groups.get(k as usize)).is_some_and(|(a, b)| {
                    a.scene.paint_operations.len() == b.scene.paint_operations.len()
                })
            };
            match run.group {
                Some(k) if whole(k) => out.boxes.push(super::BoxRange {
                    group: Some(base + k),
                    area: placed.bounds(run.area),
                    ink: run.ink.map(|ink| placed.bounds(ink)),
                    placed: Some(run.placed.map_or(placed, |p| p.then(placed))),
                    ..run.clone()
                }),
                _ => {
                    super::layer_counters::RUNS_UNPLACED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    out.unheld.push(run.owner);
                }
            }
        }
        out
    }
}

impl Window {
    /// Place a paint record `scene` that `replay_placed` refused as a copy
    /// moved by `offset` and clipped by `clip`, carrying the runs its groups
    /// hold to the record or layer painted here (`carry_grouped`).
    pub(crate) fn place_record_copy(
        &mut self,
        scene: &Scene,
        offset: Point<ScaledPixels>,
        clip: crate::Bounds<ScaledPixels>,
    ) {
        let copy = scene.copy_record(offset, Some(clip));
        let base = self.next_frame.scene.groups.len() as u32;
        if let Some(runs) = scene.book.record_runs.as_deref().filter(|_| self.takes_record_runs()) {
            // Group ids name this scene's list: only a scene whose runs index
            // it (a record's, a layer group's or the layer's own) takes them.
            let here = &self.next_frame.scene;
            let indexed = !here.in_group || here.book.record_runs.is_some() || here.book.group_runs.is_some();
            let carried = if scene.groups.is_empty() || !indexed {
                runs.dropped()
            } else {
                runs.carry_grouped(scene, &copy, base, offset)
            };
            self.deliver_record_runs(carried);
        }
        let copy = self.adopt_scene(copy);
        self.next_frame.scene.replay_record(&copy);
    }

    /// Whether the scene painted here keeps box runs: a record's, a group's
    /// of a layer, or the layer being recorded.
    pub(crate) fn takes_record_runs(&self) -> bool {
        let here = &self.next_frame.scene;
        here.book.record_runs.is_some() || here.book.group_runs.is_some() || self.layer_recording.is_some()
    }

    /// Hand `carried` to the record painted here, the group of a layer
    /// (its bindings unheld: quads in a group take none), or the layer.
    pub(crate) fn deliver_record_runs(&mut self, carried: RecordRuns) {
        let scene = &mut self.next_frame.scene;
        if let Some(own) = scene.book.record_runs.as_mut() {
            own.extend(carried);
        } else if let Some(group) = scene.book.group_runs.as_mut() {
            group.extend(carried.boxes);
            let unheld = carried.unheld.into_iter().chain(carried.bindings.iter().map(|b| b.owner));
            if let Some(recording) = self.layer_recording.as_mut() {
                recording.unheld.extend(unheld);
            }
        } else if let Some(recording) = self.layer_recording.as_mut() {
            recording.boxes.extend(carried.boxes);
            recording.bindings.extend(carried.bindings);
            recording.unheld.extend(carried.unheld);
        }
    }
}
