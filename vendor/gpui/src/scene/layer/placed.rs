//! KaminIDE patch: the box runs and value bindings a paint record carries
//! into the scenes it is placed in.
//!
//! The paint of a fragmented child (the columns of a multicol) is recorded
//! once in a scene of its own (`Window::record_paint_segments`) and placed in
//! each fragment by a shift and a clip (`Scene::replay_placed`). Without the
//! runs the record's commands reach a layer as anonymous copies: no box owns
//! them, and a paint-only edit of a box inside paints the frame again. A
//! record painted while a layer records keeps the runs of its boxes and the
//! quads their values bind (`Scene::record_runs`); every placement moves them
//! with the commands into the scene it lands in, down to the layer. A box in
//! N fragments then has N runs, each placed by its fragment's shift and clip,
//! and a later recording of the box is placed the same way (`place_run`), so
//! the layer keeps the commands a paint from scratch makes (Blink keeps
//! display items per client and fragment, `ScopedDisplayItemFragment`). A
//! record with composition groups or hoisted backdrops is placed through an
//! adopted copy (`Window::adopt_scene`) and carries nothing: its boxes are
//! not the layer's, as before.

use super::super::{PaintOperation, ScaledPixels, Scene, record};
use super::{Binding, BoxRange, ink_of};
use crate::{Bounds, Point};
use std::sync::atomic::Ordering::Relaxed;

mod replay;
mod map;
pub(crate) use replay::{place_run, release_unheld};

/// No command landed for this source command.
const NONE: u32 = u32::MAX;
/// More than one placement of the source command landed in one scene.
const MANY: u32 = u32::MAX - 1;

/// How commands came to a scene from the scene they were painted in: moved
/// by `offset`, then cut by `clip` (device pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Placed {
    pub(crate) offset: Point<ScaledPixels>,
    pub(crate) clip: Option<Bounds<ScaledPixels>>,
}

impl Placed {
    /// This placement, then `next`.
    pub(crate) fn then(self, next: Placed) -> Placed {
        let moved = self.clip.map(|c| record::geometry::bounds(c, next.offset));
        let clip = match (moved, next.clip) {
            (Some(a), Some(b)) => Some(a.intersect(&b)),
            (a, b) => a.or(b),
        };
        let offset = Point {
            x: ScaledPixels(self.offset.x.0 + next.offset.x.0),
            y: ScaledPixels(self.offset.y.0 + next.offset.y.0),
        };
        Placed { offset, clip }
    }

    fn bounds(&self, bounds: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
        let moved = record::geometry::bounds(bounds, self.offset);
        self.clip.map_or(moved, |clip| moved.intersect(&clip))
    }
}

/// Where one source command landed: its command, its quad (paint order) and
/// how it was placed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlacedOp {
    op: u32,
    quad: u32,
    placed: Placed,
}

/// Where the commands of a scene being placed land, by source command.
pub(crate) struct PlacedMap {
    /// The one placement of a replay; `None` while freezing segments, whose
    /// commands each take their route's placement.
    at: Option<Placed>,
    ops: Vec<PlacedOp>,
}

/// The box runs and bindings of a paint record, in its commands.
#[derive(Clone, Debug, Default)]
pub(crate) struct RecordRuns {
    pub(crate) boxes: Vec<BoxRange>,
    pub(crate) bindings: Vec<Binding>,
    /// Nonempty plain runs whose commands all landed nowhere. A visible or
    /// unheld copy cancels this proof when the destination layer is finished.
    pub(crate) omitted: Vec<u32>,
    /// Owners whose paint reached this scene outside their runs (a run or
    /// binding that could not be placed, a record placed as an adopted
    /// copy): the layer holds none of their runs and bindings
    /// (`SceneLayer::finish`), so an edit of such a box paints the layer
    /// anew instead of writing some of its copies and leaving the others.
    pub(crate) unheld: Vec<u32>,
}

impl RecordRuns {
    /// These runs where `map` placed their commands in `dest`. A run whose
    /// commands landed nowhere (clipped away in this fragment) is not there;
    /// the owner of one placed twice or by different placements is unheld.
    pub(crate) fn carry(&self, map: &PlacedMap, dest: &Scene) -> RecordRuns {
        let mut out = RecordRuns {
            unheld: self.unheld.clone(), omitted: self.omitted.clone(), ..RecordRuns::default()
        };
        for run in &self.boxes {
            let mut span: Option<(u32, u32)> = None;
            let mut placed: Option<Placed> = None;
            let mut whole = true;
            for source in run.ops.clone() {
                let Some(m) = map.ops.get(source as usize) else {
                    whole = false;
                    break;
                };
                if m.op == NONE {
                    continue;
                }
                whole &= m.op != MANY && placed.is_none_or(|p| p == m.placed);
                placed = Some(m.placed);
                span = Some(span.map_or((m.op, m.op), |(a, _)| (a, m.op)));
            }
            let (Some((first, last)), Some(placed), true) = (span, placed, whole) else {
                if whole && !run.ops.is_empty() && run.replaceable {
                    out.omitted.push(run.owner);
                } else {
                    super::layer_counters::RUNS_UNPLACED.fetch_add(1, Relaxed);
                    out.unheld.push(run.owner);
                }
                continue;
            };
            let ops = first..last + 1;
            let ink = dest.paint_operations[first as usize..=last as usize]
                .iter()
                .filter_map(|op| match op {
                    PaintOperation::Primitive(p) => Some(ink_of(p)),
                    _ => None,
                })
                .filter(|b| !b.is_empty())
                .reduce(|a, b| a.union(&b));
            let moved = placed.bounds(run.area);
            out.boxes.push(BoxRange {
                owner: run.owner,
                area: ink.map_or(moved, |ink| ink.union(&moved)),
                ink,
                ops,
                env: run.env,
                replaceable: run.replaceable,
                placed: Some(run.placed.map_or(placed, |p| p.then(placed))),
            });
        }
        for binding in &self.bindings {
            match map.ops.get(binding.op as usize) {
                Some(m) if m.op == NONE => {}
                Some(m) if m.op < MANY && m.quad < MANY => {
                    out.bindings.push(Binding { op: m.op, quad: m.quad, ..*binding });
                }
                _ => out.unheld.push(binding.owner),
            }
        }
        out
    }

    /// These runs placed as one copy, not by a shift and clip: every owner
    /// they hold is unheld.
    pub(crate) fn dropped(&self) -> RecordRuns {
        super::layer_counters::RUNS_UNPLACED.fetch_add(self.boxes.len() as u64, Relaxed);
        let owners = self.boxes.iter().map(|b| b.owner).chain(self.bindings.iter().map(|b| b.owner));
        let mut unheld: Vec<u32> = self.unheld.iter().chain(&self.omitted).copied().chain(owners).collect();
        unheld.sort_unstable();
        unheld.dedup();
        RecordRuns { unheld, ..RecordRuns::default() }
    }

    /// Append `other`.
    pub(crate) fn extend(&mut self, other: RecordRuns) {
        self.boxes.extend(other.boxes);
        self.bindings.extend(other.bindings);
        self.unheld.extend(other.unheld);
        self.omitted.extend(other.omitted);
    }
}

impl Scene {
    /// A record with groups or hoisted backdrops carries no runs: it is
    /// placed as one adopted copy, and the owners of the runs it kept
    /// before the group are unheld.
    pub(crate) fn end_record_runs(&mut self) {
        if !self.groups.is_empty() || !self.hoisted_backdrops.is_empty() {
            if let Some(runs) = self.book.record_runs.as_mut() {
                **runs = runs.dropped();
            }
        }
    }
}
