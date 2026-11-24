//! KaminIDE patch: the paint ranges of boxes in a scene layer.
//!
//! While a layer is recorded, the own paint of a box (its shadows, fill,
//! borders and host painter, `Window::with_run`) is painted as without
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

/// Painter responsible for a recorded owner's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerPainter {
    /// The box painter supplies all of its own paint.
    Box,
    /// The inline painter supplies one or more line fragments.
    Band,
    /// Decorations are painted by child elements.
    DecoratedBox,
    /// Other painters contribute to this owner's output.
    External,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BoxContents {
    Own,
    DecoratedBox(Pixels),
    External,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RunPainter {
    Box(BoxContents),
    Band(Option<crate::text_system::band_piece::BandEdges>),
}

impl RunPainter {
    pub(crate) fn kind(self) -> LayerPainter {
        match self {
            Self::Box(BoxContents::Own) => LayerPainter::Box,
            Self::Box(BoxContents::DecoratedBox(_)) => LayerPainter::DecoratedBox,
            Self::Box(BoxContents::External) => LayerPainter::External,
            Self::Band(_) => LayerPainter::Band,
        }
    }
}

/// The paint state a box's own paint ran in: what its later recording
/// restores.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxEnv {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) mask: ContentMask<Pixels>,
    pub(crate) transformation: TransformationMatrix,
    pub(crate) opacity: f32,
    pub(crate) rem_size: Pixels,
    /// The unrounded border box of the painting element when its snapped
    /// border box is `bounds` (`RasterFrame::exact_bounds`): border snapping
    /// and child layers read it.
    pub(crate) exact: Option<Bounds<Pixels>>,
    /// Where the host placed the box's fill (Window::box_fill_bounds).
    pub(crate) fill: Option<Bounds<Pixels>>,
    /// The order of the stacking layer the run was painted in (`None`:
    /// outside layers, ordered by the bounds tree).
    pub(crate) layer: Option<crate::scene::DrawOrder>,
    pub(crate) painter: RunPainter,
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
    /// The group of the layer whose scene holds the run (`None`: the
    /// layer's own scene); a run there is replaced in that scene
    /// (`SceneLayer::replace_group_run`).
    pub(crate) group: Option<u32>,
}

impl BoxRange {
    pub(crate) fn accepts(&self, painter: LayerPainter) -> bool {
        // A transformed run is painted again under its transformation
        // (`Window::paint_box_run`).
        self.replaceable
            && match (self.env.painter, painter) {
                (RunPainter::Box(BoxContents::Own), LayerPainter::Box) => self.env.layer.is_none(),
                (RunPainter::Band(Some(_)), LayerPainter::Band) => true,
                _ => false,
            }
    }
}

/// A box's own paint being recorded: where its commands start.
pub(crate) struct OpenBox {
    owner: u32,
    ops: usize,
    area: Bounds<ScaledPixels>,
    env: BoxEnv,
    /// The box placed more than one fill (painted per fragment of its
    /// admission): its `env` restores none of them, so the run is not
    /// replaceable and an edit of the box paints the layer anew.
    split: bool,
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
    /// (nested or bottom stacking layers, 3D, a box already open): the box
    /// paints as before. Inside one stacking layer (`Scene::push_layer`, the
    /// layer of a text line) every command takes the layer's order, which the
    /// run keeps (`BoxEnv::layer`).
    pub(crate) fn open_box(&mut self, owner: u32, area: Bounds<ScaledPixels>, mut env: BoxEnv) -> bool {
        // A group's scene (`in_group`) is composited into the layer as one
        // result: a run of its commands is no run of the layer.
        // A paint record that keeps box runs (`super::placed`) is no group.
        if self.book.open_box.is_some()
            || (self.in_group && self.book.record_runs.is_none() && self.book.group_runs.is_none())
            || self.layer_stack.len() > 1
            || self.layer_stack.last() == Some(&super::super::BOTTOM_LAYER)
            || !self.depth.plane_stack.is_empty()
            || !self.depth.contexts.is_empty()
        {
            return false;
        }
        env.layer = self.layer_stack.last().copied();
        self.book.open_box = Some(OpenBox {
            owner,
            ops: self.paint_operations.len(),
            area,
            env,
            split: false,
        });
        true
    }

    /// Whether a box's own paint is open.
    pub(crate) fn box_open(&self) -> bool {
        self.book.open_box.is_some()
    }

    /// Note a fill the open box's paint places: the first is its fill, a
    /// second splits it (`OpenBox::split`).
    pub(crate) fn note_open_box_fill(&mut self, fill: Bounds<Pixels>) {
        if let Some(open) = self.book.open_box.as_mut() {
            match open.env.fill {
                None => open.env.fill = Some(fill),
                Some(_) => open.split = true,
            }
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
                        Primitive::Shadow(_) | Primitive::Quad(_) | Primitive::Gradient(_) | Primitive::Underline(_)
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
            replaceable: plain && !open.split,
            placed: None,
            group: None,
        })
    }
}
