//! KaminIDE patch: why a box painted into a scene layer keeps no run there,
//! for the DevTools `why` journal of a host (`LayerHandle::no_run`).
//!
//! A host that cannot record a box again asks why the layer holds no run of
//! it: the box painted in a group's scene, a nested stacking layer or a 3D
//! context (`Scene::run_refusal`), or its paint left its runs
//! (`Window::unhold_owner`). Kept only while a reader traces
//! (`dirty_sources::trace::enabled`), for the last layer recorded, and bounded;
//! otherwise nothing is noted and nothing is held.

use crate::dirty_sources::trace::{Shared, enabled};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Notes kept for one layer; later ones are dropped.
const NOTES: usize = 1 << 16;

static NO_RUN: Shared<Vec<(u32, &'static str)>> = Shared::new(Vec::new());
/// The draw that recorded the notes, and the draw now (`drawn`).
static NOTED: AtomicU64 = AtomicU64::new(0);
static DRAW: AtomicU64 = AtomicU64::new(0);

/// A window draws: the layers it records replace the notes of earlier draws.
pub(crate) fn drawn() {
    if enabled() {
        DRAW.fetch_add(1, Relaxed);
    }
}

/// A layer is recorded: the first of a draw drops the notes of the last draw
/// that recorded, so the layers of one draw (the root's, lifted ones) keep theirs.
pub(crate) fn start() {
    if enabled() {
        let draw = DRAW.load(Relaxed);
        if NOTED.swap(draw, Relaxed) != draw {
            NO_RUN.lock().clear();
        }
    }
}

/// Box `owner` keeps no run of this paint, for `why`.
pub(crate) fn note(owner: u32, why: &'static str) {
    if enabled() {
        let mut notes = NO_RUN.lock();
        if notes.len() < NOTES {
            notes.push((owner, why));
        }
    }
}

impl super::LayerHandle {
    /// Why box `owner` keeps no run in the layer last recorded, when a reader
    /// traced its recording; `None` when nothing refused it a run (the frame
    /// did not paint the box) or nobody traced.
    pub fn no_run(&self, owner: u32) -> Option<&'static str> {
        NO_RUN.lock().iter().find(|(o, _)| *o == owner).map(|(_, why)| *why)
    }

    /// The painters of box `owner`'s runs, in paint order (a mixed
    /// `LayerPainter::External` named apart), for `why`.
    pub fn run_painters(&self, owner: u32) -> Vec<(crate::LayerPainter, bool)> {
        let content = self.0.content.borrow();
        content
            .runs_of(owner)
            .map(|at| {
                let run = &content.boxes[at];
                (run.env.painter.kind(), run.replaceable)
            })
            .collect()
    }
}
