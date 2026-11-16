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
        let mut out = RecordRuns { unheld: self.unheld.clone(), ..RecordRuns::default() };
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
        let mut unheld: Vec<u32> = self.unheld.iter().copied().chain(owners).collect();
        unheld.sort_unstable();
        unheld.dedup();
        RecordRuns { unheld, ..RecordRuns::default() }
    }

    /// Append `other`.
    pub(crate) fn extend(&mut self, other: RecordRuns) {
        self.boxes.extend(other.boxes);
        self.bindings.extend(other.bindings);
        self.unheld.extend(other.unheld);
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