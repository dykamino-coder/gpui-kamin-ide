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
use crate::{Bounds, Point, Window};
use std::sync::atomic::Ordering::Relaxed;

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

impl PlacedMap {
    /// A map for a replay (`at`) or for `len` commands of a draft (`None`).
    pub(crate) fn new(at: Option<Placed>, len: usize) -> Self {
        let none = PlacedOp { op: NONE, quad: NONE, placed: Placed::default() };
        let ops = if at.is_some() { Vec::new() } else { vec![none; len] };
        Self { at, ops }
    }

    /// The next source command of a replay landed nowhere.
    pub(crate) fn skip(map: &mut Option<Self>) {
        if let Some(map) = map {
            let placed = map.at.unwrap_or_default();
            map.ops.push(PlacedOp { op: NONE, quad: NONE, placed });
        }
    }

    /// The next source command of a replay landed as command `op`.
    pub(crate) fn land(map: &mut Option<Self>, op: usize, quad: Option<usize>) {
        if let Some(map) = map {
            let placed = map.at.unwrap_or_default();
            let quad = quad.map_or(NONE, |q| q as u32);
            map.ops.push(PlacedOp { op: op as u32, quad, placed });
        }
    }

    /// Source command `source` of a draft landed as command `op`.
    pub(crate) fn set(map: &mut Option<Self>, source: usize, op: usize, quad: Option<usize>, placed: Placed) {
        let Some(entry) = map.as_mut().and_then(|m| m.ops.get_mut(source)) else {
            return;
        };
        *entry = if entry.op == NONE {
            PlacedOp { op: op as u32, quad: quad.map_or(NONE, |q| q as u32), placed }
        } else {
            PlacedOp { op: MANY, quad: MANY, placed }
        };
    }
}

/// The box runs and bindings of a paint record, in its commands.
#[derive(Clone, Debug, Default)]
pub(crate) struct RecordRuns {
    pub(crate) boxes: Vec<BoxRange>,
    pub(crate) bindings: Vec<Binding>,
}

impl RecordRuns {
    /// These runs where `map` placed their commands in `dest`. A run whose
    /// commands landed nowhere (clipped away in this fragment) is not there;
    /// one placed twice or by different placements is dropped, so its box
    /// paints the layer anew instead of a run being recorded wrongly.
    pub(crate) fn carry(&self, map: &PlacedMap, dest: &Scene) -> RecordRuns {
        let mut out = RecordRuns::default();
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
                if !whole {
                    super::layer_counters::RUNS_UNPLACED.fetch_add(1, Relaxed);
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
            let Some(m) = map.ops.get(binding.op as usize) else {
                continue;
            };
            if m.op < MANY && m.quad < MANY {
                out.bindings.push(Binding { op: m.op, quad: m.quad, ..*binding });
            }
        }
        out
    }
}

impl Scene {
    /// A record with groups or hoisted backdrops carries no runs.
    pub(crate) fn end_record_runs(&mut self) {
        if !self.groups.is_empty() || !self.hoisted_backdrops.is_empty() {
            // Runs kept before the group are dropped too: the record is placed
            // as one adopted copy.
            if let Some(runs) = self.record_runs.take() {
                super::layer_counters::RUNS_UNPLACED.fetch_add(runs.boxes.len() as u64, Relaxed);
            }
        }
    }
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
    out.placed_map = Some(PlacedMap::new(Some(placed), 0));
    if !out.replay_placed(&scene, placed.offset, placed.clip) {
        return None;
    }
    let map = out.placed_map.take()?;
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
            self.next_frame.scene.record_runs = Some(Box::default());
        }
    }

    /// `Scene::replay_placed` of `scene` into the current scene, carrying the
    /// record's box runs and bindings to the record being painted here or to
    /// the layer being recorded.
    pub(crate) fn replay_placed_carrying(
        &mut self,
        scene: &Scene,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> bool {
        let here = &self.next_frame.scene;
        let takes = scene.record_runs.is_some()
            && if here.in_group { here.record_runs.is_some() } else { self.layer_recording.is_some() };
        if takes {
            self.next_frame.scene.placed_map = Some(PlacedMap::new(Some(Placed { offset, clip }), 0));
        }
        let done = self.next_frame.scene.replay_placed(scene, offset, clip);
        let map = self.next_frame.scene.placed_map.take();
        if let (true, Some(map), Some(runs)) = (done, map, scene.record_runs.as_deref()) {
            let carried = runs.carry(&map, &self.next_frame.scene);
            if let Some(own) = self.next_frame.scene.record_runs.as_mut() {
                own.boxes.extend(carried.boxes);
                own.bindings.extend(carried.bindings);
            } else if let Some(recording) = self.layer_recording.as_mut() {
                recording.boxes.extend(carried.boxes);
                recording.bindings.extend(carried.bindings);
            }
        }
        done
    }
}
