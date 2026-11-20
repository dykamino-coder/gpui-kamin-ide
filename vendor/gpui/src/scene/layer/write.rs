//! KaminIDE patch: value writes into a scene layer (`LayerHandle`).
//!
//! A value write changes a painted value of existing commands in place: the
//! fill of the quads an owner's background scope painted, with its edge
//! colour, under the painted opacity, in the quads and in their commands. A
//! change of what a box paints (a fill that appears or goes, a new border)
//! is not a value write: the host records the box's run again
//! (`Window::rerecord_layer_box`).

use super::{LayerHandle, LayerValue, SceneLayer, layer_counters};
use crate::scene::{Background, Bounds, Hsla, PaintOperation, Primitive, ScaledPixels};
use std::ops::Range;
use std::sync::atomic::Ordering::Relaxed;

impl SceneLayer {
    fn bound(&self, owner: u32, value: LayerValue) -> Range<usize> {
        let start = self
            .bindings
            .partition_point(|b| (b.owner, b.value) < (owner, value));
        let end = self
            .bindings
            .partition_point(|b| (b.owner, b.value) <= (owner, value));
        start..end
    }

    /// Whether every quad the owner's background scope painted has `from`.
    fn painted_in(&self, owner: u32, from: Hsla) -> bool {
        let range = self.bound(owner, LayerValue::Background);
        !range.is_empty()
            && self.bindings[range].iter().all(|b| {
                let fill = Background::from(from).opacity(b.opacity);
                self.scene
                    .quads
                    .get(b.quad as usize)
                    .is_some_and(|q| q.background == fill)
            })
    }
}

impl LayerHandle {
    /// Whether box `owner` painted runs this layer can record again
    /// (`Window::rerecord_layer_box`): at least one, each of plain
    /// primitives under no transformation, outside stacking layers (the runs
    /// of an inline box' band on its line layer take value writes only).
    pub fn holds_box(&self, owner: u32) -> bool {
        self.accepts(owner, crate::LayerPainter::Box)
    }

    /// Whether every piece of the owner's band can be recorded again.
    pub fn holds_band(&self, owner: u32) -> bool {
        self.accepts(owner, crate::LayerPainter::Band)
    }

    /// Painter of the runs actually recorded in this layer. Mixed runs need a full repaint.
    pub fn painter(&self, owner: u32) -> Option<crate::LayerPainter> {
        let content = self.0.content.borrow();
        let mut kinds = content.runs_of(owner).map(|at| content.boxes[at].env.painter.kind());
        let first = kinds.next()?;
        Some(if kinds.all(|kind| kind == first) { first } else { crate::LayerPainter::External })
    }

    pub(crate) fn accepts(&self, owner: u32, painter: crate::LayerPainter) -> bool {
        let content = self.0.content.borrow();
        let mut runs = content.runs_of(owner).peekable();
        runs.peek().is_some() && runs.all(|at| content.boxes[at].accepts(painter))
    }

    /// The device-pixel area the runs of box `owner` cover, their placed box
    /// with its ink (`BoxRange::area`): any paint the box records again in
    /// the same place lies inside it. `None` when the box has no run.
    pub fn box_area(&self, owner: u32) -> Option<Bounds<ScaledPixels>> {
        let content = self.0.content.borrow();
        content
            .runs_of(owner)
            .map(|at| content.boxes[at].area)
            .reduce(|a, b| a.union(&b))
    }

    /// Bounds of the old and proposed decoration paint in the layer's coordinates.
    /// Grow the original border box before any clip. A clipped or empty own run
    /// can still have a child outline that reaches the viewport.
    pub fn decoration_area(
        &self,
        owner: u32,
        proposed: crate::Pixels,
        scale: f32,
    ) -> Option<Bounds<ScaledPixels>> {
        use super::{BoxContents, RunPainter};
        let content = self.0.content.borrow();
        let mut area = None;
        for at in content.runs_of(owner) {
            let run = &content.boxes[at];
            if run.env.transformation != crate::TransformationMatrix::unit() {
                return None;
            }
            let outset = match run.env.painter {
                RunPainter::Box(BoxContents::Own) => proposed,
                RunPainter::Box(BoxContents::DecoratedBox(old)) => proposed.max(old),
                _ => return None,
            };
            let mut bounds = run.env.bounds.dilate(outset).scale(scale);
            if let Some(placed) = run.placed {
                bounds.origin = bounds.origin + placed.offset;
            }
            let bounds = bounds.union(&run.area);
            area = Some(area.map_or(bounds, |other: Bounds<_>| other.union(&bounds)));
        }
        area
    }

    /// Write the background of `owner` from `from` to `to`.
    /// `set_backgrounds` of one write.
    pub fn set_background(&self, owner: u32, from: Hsla, to: Hsla) -> Option<Bounds<ScaledPixels>> {
        self.set_backgrounds(&[(owner, Some(from), Some(to))])?
    }

    /// Write the backgrounds `(owner, from, to)`: every quad an owner's
    /// background scope painted takes `to` (under the opacity it was painted
    /// with). All or nothing: `None`, with nothing written, when an owner has
    /// no such quad, one of them was painted in another colour than `from`,
    /// or a side has no fill (`None` or transparent: the painter paints no
    /// quad then); the host records the box again. Otherwise the damaged
    /// rectangle in device pixels (`None` inside for no write), also added to
    /// the layer's damage for the next frame that draws it.
    pub fn set_backgrounds(
        &self,
        writes: &[(u32, Option<Hsla>, Option<Hsla>)],
    ) -> Option<Option<Bounds<ScaledPixels>>> {
        let opaque = |c: Option<Hsla>| c.filter(|c| c.a > 0.);
        let mut current = self.0.content.borrow_mut();
        let mut plain = Vec::with_capacity(writes.len());
        for &(owner, from, to) in writes {
            let (from, to) = (opaque(from)?, opaque(to)?);
            if from != to {
                if !current.painted_in(owner, from) {
                    return None;
                }
                plain.push((owner, from, to));
            }
        }
        if plain.is_empty() {
            return Some(None);
        }
        let layer = std::rc::Rc::make_mut(&mut current);
        let mut damage: Option<Bounds<ScaledPixels>> = None;
        let mut written = 0;
        // A fill's quad takes the fill's colour as its transparent edge (the
        // box painter's fill quad), which follows the fill; any other edge
        // (a text band's transparent black) stays as painted.
        let edge_of = |c: Hsla, opacity: f32| Hsla { a: 0., ..c }.opacity(opacity);
        for &(owner, from, to) in &plain {
            for at in layer.bound(owner, LayerValue::Background) {
                let b = layer.bindings[at];
                let fill = Background::from(to).opacity(b.opacity);
                let (old, border) = (edge_of(from, b.opacity), edge_of(to, b.opacity));
                let edge = |e: &mut Hsla| {
                    if *e == old {
                        *e = border;
                    }
                };
                if let Some(quad) = layer.scene.quads.get_mut(b.quad as usize) {
                    quad.background = fill;
                    edge(&mut quad.border_color);
                    let rect = super::boxes::ink_of(&Primitive::Quad(*quad));
                    damage = Some(damage.map_or(rect, |d| d.union(&rect)));
                }
                if let Some(PaintOperation::Primitive(Primitive::Quad(quad))) =
                    layer.scene.paint_operations.get_mut(b.op as usize)
                {
                    quad.background = fill;
                    edge(&mut quad.border_color);
                }
                written += 1;
            }
        }
        layer.version += 1;
        layer_counters::VALUES_WRITTEN.fetch_add(written, Relaxed);
        if let Some(rect) = damage {
            let old = self.0.damage.get();
            self.0
                .damage
                .set(Some(old.map_or(rect, |d| d.union(&rect))));
        }
        Some(damage)
    }
}
