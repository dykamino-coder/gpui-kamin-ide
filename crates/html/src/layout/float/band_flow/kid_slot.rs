//! A child's probe results between frames, on the child's own box.
//!
//! A probe lays a child out at an available inline size in a trial tree and
//! reads its size, overflow, geometry and clamp. The answer is a function of
//! the child's subtree, its kind, the containing-block and available widths,
//! the float exclusions it wraps around, the writing mode and what its host's
//! build reads from outside (`BandKey`). A later frame whose child box is clean
//! (`BoxTree::build_epoch` of the child) takes the same answer without
//! building the child, even when a sibling changed and its host is laid out
//! again: the edit costs the probes of the dirty child only, as a Blink
//! `LayoutResult` is reused per box by its constraint space. A hit has no
//! prepared element; the host's holder builds the child once at its place.

use super::kid::{Kid, Kind};
use super::{HostSlot, Probe, Shapes};
use crate::layout::fragment::flow::FlowGeometry;
use crate::text::clamp::MeasuredClamp;
use gpui::{Bounds, Pixels};
use std::sync::Arc;

/// Answers kept per child: the widths of one host's plans and probes.
const PROBES: usize = 8;

/// The input of one probe of a child.
#[derive(Clone)]
pub(super) struct ProbeInput {
    pub(super) kind: Kind,
    pub(super) cb: f32,
    pub(super) avail: f32,
    pub(super) shapes: Option<Shapes>,
    pub(super) vert: bool,
}

impl ProbeInput {
    /// The input of a probe of `kid`, when its host keeps results.
    pub(super) fn of(
        kid: &Kid,
        cb: f32,
        avail: f32,
        shapes: Option<Shapes>,
        p: &Probe,
    ) -> Option<Self> {
        p.kept.as_ref()?;
        Some(Self {
            kind: kid.kind,
            cb,
            avail,
            shapes,
            vert: p.vert,
        })
    }

    fn same(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.cb == other.cb
            && self.avail == other.avail
            && self.vert == other.vert
            && match (&self.shapes, &other.shapes) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
                _ => false,
            }
    }
}

/// What a probe of a child gave.
#[derive(Clone)]
pub(super) struct ProbeOutput {
    pub(super) size: (f32, f32),
    pub(super) overflow: Bounds<Pixels>,
    pub(super) geometry: FlowGeometry,
    pub(super) clamp: MeasuredClamp,
}

/// The kept probes of a child box (`LayoutSlot::probe`).
#[derive(Default)]
pub(crate) struct KidProbes {
    entries: Vec<(ProbeInput, ProbeOutput)>,
    next: usize,
    pub(super) intrinsic: Option<(f32, f32)>,
}

impl KidProbes {
    fn find(&self, input: &ProbeInput) -> Option<ProbeOutput> {
        let found = self.entries.iter().find(|(i, _)| i.same(input));
        found.map(|(_, o)| o.clone())
    }

    fn keep(&mut self, input: ProbeInput, output: ProbeOutput) {
        if self.entries.iter().any(|(i, _)| i.same(&input)) {
            return;
        }
        if self.entries.len() < PROBES {
            self.entries.push((input, output));
        } else {
            self.entries[self.next] = (input, output);
            self.next = (self.next + 1) % PROBES;
        }
    }
}

impl HostSlot {
    /// The kept answer of the probe `input` of the child with node `node`.
    pub(super) fn probe(&self, node: u64, input: &ProbeInput) -> Option<ProbeOutput> {
        let out = self.kid(node, |kept| kept.find(input))?;
        crate::work::add!(LayoutCacheHits, 1);
        Some(out)
    }

    /// Keep the answer of a probe of this frame.
    pub(super) fn keep_probe(&self, node: u64, input: ProbeInput, output: ProbeOutput) {
        self.write_kid(node, |kept| kept.keep(input, output));
    }

    /// The child's kept min/max-content pair.
    pub(super) fn kid_intrinsic(&self, node: u64) -> Option<(f32, f32)> {
        self.kid(node, |kept| kept.intrinsic)
    }

    /// Keep the child's min/max-content pair of this frame.
    pub(super) fn keep_kid_intrinsic(&self, node: u64, sizes: (f32, f32)) {
        self.write_kid(node, |kept| kept.intrinsic = Some(sizes));
    }
}

impl Probe<'_> {
    /// The kept answer of the probe `input` of `kid`, set as this probe's
    /// outputs; it has no prepared element.
    pub(super) fn kept_probe(
        &mut self,
        kid: &Kid,
        input: Option<&ProbeInput>,
    ) -> Option<(f32, f32)> {
        let out = self.kept.as_ref()?.probe(kid.node_id, input?)?;
        self.last_overflow = out.overflow;
        self.last_geometry = out.geometry;
        self.last_clamp = out.clamp;
        self.last_prepared = Default::default();
        Some(out.size)
    }

    /// Keep this probe's outputs of size `size` for `kid`.
    pub(super) fn keep_kid_probe(&self, kid: &Kid, input: Option<ProbeInput>, size: (f32, f32)) {
        let (Some(slot), Some(input)) = (&self.kept, input) else {
            return;
        };
        let out = ProbeOutput {
            size,
            overflow: self.last_overflow,
            geometry: self.last_geometry.clone(),
            clamp: self.last_clamp.clone(),
        };
        slot.keep_probe(kid.node_id, input, out);
    }
}

impl std::fmt::Debug for KidProbes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KidProbes")
            .field("entries", &self.entries.len())
            .finish()
    }
}
