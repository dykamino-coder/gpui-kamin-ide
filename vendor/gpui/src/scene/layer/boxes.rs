//! KaminIDE patch: the paint ranges of boxes in a scene layer.
//!
//! While a layer is recorded, the own paint of a box (its shadows, fill,
//! borders and host painter, `Window::with_box_paint`) is painted as without
//! a layer: each command takes its order from the scene's bounds tree when it
//! is painted. The layer only notes the box's run of commands, its ink and
//! paint state, so a later recording of the box replaces the run whole
//! (`Window::rerecord_layer_box`) with the orders a paint from scratch gives
//! the new commands (`scene::layer::order`). Blink keeps the display items of
//! a client in paint order and repaints a client's items into a new artifact
//! (`paint_controller.cc:228`); this layer splices the box's run in place.

use super::super::{PaintOperation, Primitive, ScaledPixels, Scene, placed_bounds};
use crate::{Bounds, ContentMask, Pixels, TransformationMatrix};
use std::ops::Range;

/// The paint state a box's own paint ran in: what its later recording
/// restores.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxEnv {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) mask: ContentMask<Pixels>,
    pub(crate) transformation: TransformationMatrix,
    pub(crate) opacity: f32,
    pub(crate) rem_size: Pixels,
    /// Where the host placed the box's fill (Window::box_fill_bounds).
    pub(crate) fill: Option<Bounds<Pixels>>,
}

/// A box's own paint in a layer: its owner, its placed area and ink, its run
/// of paint commands and its environment.
#[derive(Clone, Debug)]
pub(crate) struct BoxRange {
    pub(crate) owner: u32,
    pub(crate) area: Bounds<ScaledPixels>,
    pub(crate) ink: Option<Bounds<ScaledPixels>>,
    pub(crate) ops: Range<u32>,
    pub(crate) env: BoxEnv,
    /// The run is plain primitives, which a recording replaces.
    pub(crate) replaceable: bool,
    /// How the run's commands came here from the paint record its box painted
    /// in (`super::placed`); a recording of the box is placed the same way.
    pub(crate) placed: Option<super::Placed>,
}

/// A box's own paint being recorded: where its commands start.
pub(crate) struct OpenBox {
    owner: u32,
    ops: usize,
    area: Bounds<ScaledPixels>,
    env: BoxEnv,
}

/// The clipped placed bounds of a primitive.
pub(crate) fn ink_of(primitive: &Primitive) -> Bounds<ScaledPixels> {
    placed_bounds(primitive).intersect(&primitive.content_mask().bounds)
}

fn union(a: Option<Bounds<ScaledPixels>>, b: Bounds<ScaledPixels>) -> Option<Bounds<ScaledPixels>> {
    if b.is_empty() {
        return a;
    }
    Some(a.map_or(b, |a| a.union(&b)))
}

impl Scene {
    /// Start the own paint of box `owner` with `area` (its placed bounds,
    /// device pixels). `false` where the scene orders by other means
    /// (stacking layers, 3D, a box already open): the box paints as before.
    pub(crate) fn open_box(&mut self, owner: u32, area: Bounds<ScaledPixels>, env: BoxEnv) -> bool {
        // A group's scene (`in_group`) is composited into the layer as one
        // result: a run of its commands is no run of the layer.
        // A paint record that keeps box runs (`super::placed`) is no group.
        if self.book.open_box.is_some()
            || (self.in_group && self.book.record_runs.is_none())
            || !self.layer_stack.is_empty()
            || !self.depth.plane_stack.is_empty()
            || !self.depth.contexts.is_empty()
        {
            return false;
        }
        self.book.open_box = Some(OpenBox {
            owner,
            ops: self.paint_operations.len(),
            area,
            env,
        });
        true
    }

    /// Whether a box's own paint is open.
    pub(crate) fn box_open(&self) -> bool {
        self.book.open_box.is_some()
    }

    /// The fill bounds the open box's paint placed.
    pub(crate) fn open_box_fill(&self) -> Option<Bounds<Pixels>> {
        self.book.open_box.as_ref()?.env.fill
    }

    pub(crate) fn set_open_box_fill(&mut self, fill: Bounds<Pixels>) {
        if let Some(open) = self.book.open_box.as_mut() {
            open.env.fill = Some(fill);
        }
    }

    /// Finish the open box: its run of commands and their ink.
    pub(crate) fn close_box(&mut self) -> Option<BoxRange> {
        let open = self.book.open_box.take()?;
        let ops = open.ops..self.paint_operations.len();
        let mut ink = None;
        let mut plain = true;
        for operation in &self.paint_operations[ops.clone()] {
            match operation {
                PaintOperation::Primitive(p) => {
                    // Sprites sort by tile within an order: not in paint order.
                    plain &= matches!(
                        p,
                        Primitive::Shadow(_) | Primitive::Quad(_) | Primitive::Underline(_)
                    );
                    ink = union(ink, ink_of(p));
                }
                _ => plain = false,
            }
        }
        Some(BoxRange {
            owner: open.owner,
            area: union(ink, open.area).unwrap_or(open.area),
            ink,
            ops: ops.start as u32..ops.end as u32,
            env: open.env,
            replaceable: plain,
            placed: None,
        })
    }
}
