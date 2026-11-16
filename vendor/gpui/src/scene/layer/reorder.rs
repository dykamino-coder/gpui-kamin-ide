//! KaminIDE patch: later commands of a scene layer moved to the buckets a
//! paint from scratch gives them after a box's run changed
//! (`scene::layer::order`), and the sorted lists built again from the
//! commands in paint order, as the recording built them.

use super::SceneLayer;
use super::boxes::ink_of;
use crate::scene::{DrawOrder, ORDER_STEP, PaintOperation, Primitive, Scene};
use crate::{Bounds, ScaledPixels};

fn union(a: Option<Bounds<ScaledPixels>>, b: Bounds<ScaledPixels>) -> Option<Bounds<ScaledPixels>> {
    if b.is_empty() {
        return a;
    }
    Some(a.map_or(b, |a| a.union(&b)))
}

impl SceneLayer {
    /// Whether each sorted list holds exactly the primitives of the
    /// commands, as a recording leaves them: lists built again from the
    /// commands are the same lists in another order.
    pub(super) fn lists_follow_commands(&self) -> bool {
        let s = &self.scene;
        let mut n = [0usize; 8];
        for op in &s.paint_operations {
            if let PaintOperation::Primitive(p) = op {
                n[slot(p)] += 1;
            }
        }
        n == [
            s.shadows.len(),
            s.quads.len(),
            s.paths.len(),
            s.underlines.len(),
            s.monochrome_sprites.len(),
            s.subpixel_sprites.len(),
            s.polychrome_sprites.len(),
            s.surfaces.len(),
        ] && s.layers.is_empty()
    }

    /// Give the commands `later` (`(command, bucket)`: primitives outside
    /// layers and layers) their new orders, a layer's own primitives with it,
    /// and sort the lists again; the ink whose order changed.
    pub(super) fn move_later(&mut self, later: &[(usize, DrawOrder)]) -> Option<Bounds<ScaledPixels>> {
        let ops = &mut self.scene.paint_operations;
        let mut ink = None;
        for &(at, bucket) in later {
            let order = bucket * ORDER_STEP;
            let layer = match &mut ops[at] {
                PaintOperation::Primitive(p) => {
                    *p.order_mut() = order;
                    ink = union(ink, ink_of(p));
                    false
                }
                PaintOperation::StartLayer(bounds, o) => {
                    *o = order;
                    ink = union(ink, *bounds);
                    true
                }
                _ => false,
            };
            if !layer {
                continue;
            }
            // The layer's own primitives, not those of the layers inside it.
            let mut depth = 0usize;
            for op in &mut ops[at + 1..] {
                match op {
                    PaintOperation::StartLayer(..) | PaintOperation::StartBottomLayer => depth += 1,
                    PaintOperation::EndLayer if depth == 0 => break,
                    PaintOperation::EndLayer => depth -= 1,
                    PaintOperation::Primitive(p) if depth == 0 => {
                        *p.order_mut() = order;
                        ink = union(ink, ink_of(p));
                    }
                    _ => {}
                }
            }
        }
        self.sort_again();
        ink
    }

    /// Build the sorted lists from the commands in paint order and sort them
    /// as `Scene::finish` does; bindings follow their commands' quads.
    fn sort_again(&mut self) {
        let Scene {
            paint_operations,
            shadows,
            quads,
            paths,
            underlines,
            monochrome_sprites,
            subpixel_sprites,
            polychrome_sprites,
            surfaces,
            ..
        } = &mut self.scene;
        shadows.clear();
        quads.clear();
        paths.clear();
        underlines.clear();
        monochrome_sprites.clear();
        subpixel_sprites.clear();
        polychrome_sprites.clear();
        surfaces.clear();
        // The paint index of each command's quad.
        let mut quad_of = vec![u32::MAX; paint_operations.len()];
        for (at, op) in paint_operations.iter().enumerate() {
            let PaintOperation::Primitive(p) = op else { continue };
            match p {
                Primitive::Shadow(x) => shadows.push(*x),
                Primitive::Quad(x) => {
                    quad_of[at] = quads.len() as u32;
                    quads.push(*x);
                }
                Primitive::Path(x) => paths.push(x.clone()),
                Primitive::Underline(x) => underlines.push(*x),
                Primitive::MonochromeSprite(x) => monochrome_sprites.push(x.clone()),
                Primitive::SubpixelSprite(x) => subpixel_sprites.push(x.clone()),
                Primitive::PolychromeSprite(x) => polychrome_sprites.push(x.clone()),
                Primitive::Surface(x) => surfaces.push(x.clone()),
            }
        }
        let moved = self.scene.finish_tracking_quads();
        for b in &mut self.bindings {
            if let Some(&q) = quad_of.get(b.op as usize).filter(|&&q| q != u32::MAX) {
                b.quad = moved[q as usize];
            }
        }
        self.bindings.sort_by_key(|x| (x.owner, x.value, x.quad));
    }
}

fn slot(p: &Primitive) -> usize {
    match p {
        Primitive::Shadow(_) => 0,
        Primitive::Quad(_) => 1,
        Primitive::Path(_) => 2,
        Primitive::Underline(_) => 3,
        Primitive::MonochromeSprite(_) => 4,
        Primitive::SubpixelSprite(_) => 5,
        Primitive::PolychromeSprite(_) => 6,
        Primitive::Surface(_) => 7,
    }
}
