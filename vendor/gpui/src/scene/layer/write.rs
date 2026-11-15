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
        let start = self.bindings.partition_point(|b| (b.owner, b.value) < (owner, value));
        let end = self.bindings.partition_point(|b| (b.owner, b.value) <= (owner, value));
        start..end
    }

    /// Whether every quad the owner's background scope painted has `from`.
    fn painted_in(&self, owner: u32, from: Hsla) -> bool {
        let range = self.bound(owner, LayerValue::Background);
        !range.is_empty()
            && self.bindings[range].iter().all(|b| {
                let fill = Background::from(from).opacity(b.opacity);
                self.scene.quads.get(b.quad as usize).is_some_and(|q| q.background == fill)
            })
    }
}

impl LayerHandle {
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
    pub fn set_backgrounds(&self, writes: &[(u32, Option<Hsla>, Option<Hsla>)]) -> Option<Option<Bounds<ScaledPixels>>> {
        let opaque = |c: Option<Hsla>| c.filter(|c| c.a > 0.);
        let mut current = self.0.content.borrow_mut();
        let mut plain = Vec::with_capacity(writes.len());
        for &(owner, from, to) in writes {
            let (from, to) = (opaque(from)?, opaque(to)?);
            if from != to {
                if !current.painted_in(owner, from) {
                    return None;
                }
                plain.push((owner, to));
            }
        }
        if plain.is_empty() {
            return Some(None);
        }
        let layer = std::rc::Rc::make_mut(&mut current);
        let mut damage: Option<Bounds<ScaledPixels>> = None;
        let mut written = 0;
        for &(owner, to) in &plain {
            let mut edge = to;
            edge.a = 0.;
            for at in layer.bound(owner, LayerValue::Background) {
                let b = layer.bindings[at];
                let fill = Background::from(to).opacity(b.opacity);
                let border = edge.opacity(b.opacity);
                if let Some(quad) = layer.scene.quads.get_mut(b.quad as usize) {
                    quad.background = fill;
                    quad.border_color = border;
                    let rect = super::boxes::ink_of(&Primitive::Quad(*quad));
                    damage = Some(damage.map_or(rect, |d| d.union(&rect)));
                }
                if let Some(PaintOperation::Primitive(Primitive::Quad(quad))) = layer.scene.paint_operations.get_mut(b.op as usize) {
                    quad.background = fill;
                    quad.border_color = border;
                }
                written += 1;
            }
        }
        layer.version += 1;
        layer_counters::VALUES_WRITTEN.fetch_add(written, Relaxed);
        if let Some(rect) = damage {
            let old = self.0.damage.get();
            self.0.damage.set(Some(old.map_or(rect, |d| d.union(&rect))));
        }
        Some(damage)
    }
}
