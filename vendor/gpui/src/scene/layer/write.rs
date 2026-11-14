//! KaminIDE patch: value writes into a scene layer (`LayerHandle`).
//!
//! A background write changes the fill of the quads an owner's background
//! scope painted, takes them out when the fill becomes transparent, and puts
//! a quad at the owner's reserved place when it had none (the painter paints
//! no transparent fill, `Style::paint_box`). Each case leaves the layer's
//! scene as a fresh recording of the new values would be: quads sorted by
//! order, the fill colour with its edge colour, under the painted opacity.
//! The paint commands keep the new fill where one exists; a quad put at a
//! reserved place has a command only if a write took it out before. Ranges
//! of the commands replay into later recordings only for content the host
//! did not restyle, so no range copies a stale fill.

use super::{Binding, LayerHandle, LayerValue, Reserved, SceneLayer, layer_counters};
use crate::scene::{Background, Bounds, Hsla, PaintOperation, Primitive, Quad, ScaledPixels};
use std::ops::Range;
use std::sync::atomic::Ordering::Relaxed;

impl SceneLayer {
    fn bound(&self, owner: u32, value: LayerValue) -> Range<usize> {
        let start = self.bindings.partition_point(|b| (b.owner, b.value) < (owner, value));
        let end = self.bindings.partition_point(|b| (b.owner, b.value) <= (owner, value));
        start..end
    }

    fn places(&self, owner: u32, value: LayerValue) -> Range<usize> {
        let start = self.reserved.partition_point(|r| (r.owner, r.value) < (owner, value));
        let end = self.reserved.partition_point(|r| (r.owner, r.value) <= (owner, value));
        start..end
    }

    /// Whether `(owner, from, to)` applies: the owner's painted quads all
    /// have `from`, or it has places and painted nothing (`from` is `None`).
    fn applies(&self, owner: u32, from: Option<Hsla>, to: Option<Hsla>) -> bool {
        let painted = self.bound(owner, LayerValue::Background);
        if !painted.is_empty() {
            let Some(from) = from else { return false };
            return self.bindings[painted].iter().all(|b| {
                let fill = Background::from(from).opacity(b.opacity);
                self.scene.quads.get(b.quad as usize).is_some_and(|q| q.background == fill)
            });
        }
        let places = self.places(owner, LayerValue::Background);
        if places.is_empty() || from.is_some() {
            return false;
        }
        // A new quad must lie inside the layer's bounds, by which the frame
        // ordered the layer among its own primitives.
        to.is_none()
            || self.reserved[places].iter().all(|r| {
                let rect = super::super::placed_bounds(&Primitive::Quad(r.quad)).intersect(&r.quad.content_mask.bounds);
                self.bounds.is_some_and(|b| b.intersect(&rect) == rect)
            })
    }

    fn fill(quad: &mut Quad, to: Option<Hsla>, opacity: f32) {
        let colour = to.unwrap_or_default();
        let mut edge = colour;
        edge.a = 0.;
        quad.background = Background::from(colour).opacity(opacity);
        quad.border_color = edge.opacity(opacity);
    }

    fn fill_command(&mut self, op: u32, to: Option<Hsla>, opacity: f32) {
        if let Some(PaintOperation::Primitive(Primitive::Quad(quad))) = self.scene.paint_operations.get_mut(op as usize) {
            Self::fill(quad, to, opacity);
        }
    }

    /// Write `to` into the owner's quads (`applies` held); the damage.
    fn write_background(&mut self, owner: u32, to: Option<Hsla>) -> Option<Bounds<ScaledPixels>> {
        let painted = self.bound(owner, LayerValue::Background);
        let mut damage = None;
        let mut add = |rect: Bounds<ScaledPixels>| damage = Some(damage.map_or(rect, |d: Bounds<_>| d.union(&rect)));
        let rect_of = |q: &Quad| super::super::placed_bounds(&Primitive::Quad(*q)).intersect(&q.content_mask.bounds);
        if !painted.is_empty() && to.is_some() {
            for at in painted {
                let b = self.bindings[at];
                if let Some(quad) = self.scene.quads.get_mut(b.quad as usize) {
                    Self::fill(quad, to, b.opacity);
                    add(rect_of(quad));
                }
                self.fill_command(b.op, to, b.opacity);
            }
        } else if !painted.is_empty() {
            // The fill became transparent: its quads leave for reserved places.
            let mut gone: Vec<Binding> = self.bindings.drain(painted).collect();
            gone.sort_by_key(|b| std::cmp::Reverse(b.quad));
            for b in gone {
                let mut quad = self.scene.quads.remove(b.quad as usize);
                add(rect_of(&quad));
                self.fill_command(b.op, None, b.opacity);
                Self::fill(&mut quad, None, b.opacity);
                for other in &mut self.bindings {
                    other.quad -= u32::from(other.quad > b.quad);
                }
                self.reserved.push(Reserved { owner, value: b.value, quad, op: b.op, opacity: b.opacity });
            }
            self.reserved.sort_by_key(|r| (r.owner, r.value, r.quad.order));
        } else if to.is_some() {
            let places: Vec<Reserved> = self.reserved.drain(self.places(owner, LayerValue::Background)).collect();
            for r in places {
                let mut quad = r.quad;
                Self::fill(&mut quad, to, r.opacity);
                add(rect_of(&quad));
                self.fill_command(r.op, to, r.opacity);
                // After the quads of equal order: the fill was painted last.
                let at = self.scene.quads.partition_point(|q| q.order <= quad.order);
                self.scene.quads.insert(at, quad);
                for other in &mut self.bindings {
                    other.quad += u32::from(other.quad as usize >= at);
                }
                self.bindings.push(Binding { owner, value: r.value, quad: at as u32, op: r.op, opacity: r.opacity });
            }
            self.bindings.sort_by_key(|b| (b.owner, b.value, b.quad));
        }
        damage
    }
}

impl LayerHandle {
    /// Write the background of `owner` from `from` to `to` (both painted,
    /// not transparent). `set_backgrounds` of one write.
    pub fn set_background(&self, owner: u32, from: Hsla, to: Hsla) -> Option<Bounds<ScaledPixels>> {
        self.set_backgrounds(&[(owner, Some(from), Some(to))])?
    }

    /// Write the backgrounds `(owner, from, to)`, `None` or a transparent
    /// colour meaning no fill: every quad an owner's background scope painted
    /// takes `to` (under the opacity it was painted with) or leaves, and an
    /// owner that painted none gets a quad at each place it reserved. All or
    /// nothing: `None`, with nothing written, when an owner has neither quads
    /// in `from` nor places for a `from` without fill; the host paints again.
    /// Otherwise the damaged rectangle in device pixels (`None` inside for no
    /// write), also added to the layer's damage for the next frame drawing it.
    pub fn set_backgrounds(&self, writes: &[(u32, Option<Hsla>, Option<Hsla>)]) -> Option<Option<Bounds<ScaledPixels>>> {
        let opaque = |c: Option<Hsla>| c.filter(|c| c.a > 0.);
        let writes: Vec<_> = writes
            .iter()
            .map(|&(owner, from, to)| (owner, opaque(from), opaque(to)))
            .filter(|&(_, from, to)| from != to)
            .collect();
        let mut current = self.0.content.borrow_mut();
        if !writes.iter().all(|&(owner, from, to)| current.applies(owner, from, to)) {
            return None;
        }
        if writes.is_empty() {
            return Some(None);
        }
        let layer = std::rc::Rc::make_mut(&mut current);
        let mut damage: Option<Bounds<ScaledPixels>> = None;
        for &(owner, _, to) in &writes {
            if let Some(rect) = layer.write_background(owner, to) {
                damage = Some(damage.map_or(rect, |d| d.union(&rect)));
            }
        }
        layer.version += 1;
        layer_counters::VALUES_WRITTEN.fetch_add(writes.len() as u64, Relaxed);
        if let Some(rect) = damage {
            let old = self.0.damage.get();
            self.0.damage.set(Some(old.map_or(rect, |d| d.union(&rect))));
        }
        Some(damage)
    }
}
