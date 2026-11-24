//! KaminIDE patch: a primitive of the new frame against one of the last frame
//! moved by `d` (`plan_shift`), both cut by the moved window `W`.

use super::super::{Area, KIND_SHIFT, clipped, same_path};
use super::Item;
use crate::color::BackgroundTag;
use crate::scene::{Quad, Scene};
use crate::{Bounds, ContentMask, Point, ScaledPixels, TransformationMatrix, point, size};
use collections::FxHashMap;

/// The comparison of one plan: the move, `W` and the exact results by item pair.
pub(super) struct Moved {
    pub(super) d: (f32, f32),
    pub(super) w: Area,
    pub(super) exact: FxHashMap<(u32, u32), bool>,
}

fn at(b: Bounds<ScaledPixels>, d: (f32, f32)) -> Bounds<ScaledPixels> {
    Bounds {
        origin: b.origin + point(ScaledPixels(d.0), ScaledPixels(d.1)),
        size: b.size,
    }
}

/// Move a primitive's place: its bounds, or with a transformation the
/// translation the transformation applies after them.
fn place(b: &mut Bounds<ScaledPixels>, m: &mut TransformationMatrix, d: (f32, f32)) {
    if *m == TransformationMatrix::unit() {
        *b = at(*b, d);
    } else {
        m.translation[0] += d.0;
        m.translation[1] += d.1;
    }
}

/// A fill whose pixels follow its bounds: the shader reads the position
/// relative to them (solid, gradients), not to the window.
fn movable(q: &Quad) -> bool {
    matches!(
        q.background.tag,
        BackgroundTag::Solid
            | BackgroundTag::LinearGradient
            | BackgroundTag::RadialGradient
            | BackgroundTag::AnalyticGradient
    )
}

/// `b ⊇ p` for the integer rectangle `p` (x0, y0, x1, y1): every pixel centre
/// of `p` lies inside `b` at least half a pixel from its edges, so an edge
/// of `b` antialiases none of them.
fn covers(b: Bounds<ScaledPixels>, p: [u32; 4]) -> bool {
    let (o, s) = (b.origin, b.size);
    o.x.0 <= p[0] as f32
        && o.y.0 <= p[1] as f32
        && o.x.0 + s.width.0 >= p[2] as f32
        && o.y.0 + s.height.0 >= p[3] as f32
}

impl Moved {
    fn window(&self) -> Bounds<ScaledPixels> {
        let w = self.w;
        Bounds {
            origin: point(ScaledPixels(w[0]), ScaledPixels(w[1])),
            size: size(ScaledPixels(w[2] - w[0]), ScaledPixels(w[3] - w[1])),
        }
    }

    fn cut(&self, m: &mut ContentMask<ScaledPixels>) {
        m.bounds = m.bounds.intersect(&self.window());
    }

    fn shift(&self, m: &mut ContentMask<ScaledPixels>) {
        m.bounds = at(m.bounds, self.d);
        self.cut(m);
    }

    /// Item `x` (new frame) paints within cell part `p` what item `y` (last
    /// frame) paints there once moved.
    pub(super) fn same(&mut self, x: (u32, Item), y: (u32, Item), p: [u32; 4]) -> bool {
        let exact = match self.exact.get(&(x.0, y.0)) {
            Some(&known) => known,
            None => {
                let known = self.exact_pair(x.1, y.1);
                self.exact.insert((x.0, y.0), known);
                known
            }
        };
        exact || (x.1.entry >> KIND_SHIFT == 1 && self.same_fill(x.1, y.1, p))
    }

    /// Two plain fills of one paint, each covering `p` (a stationary page
    /// background under a scroller: equal wherever both cover the cell).
    fn same_fill(&self, x: Item, y: Item, p: [u32; 4]) -> bool {
        let mask = (1 << KIND_SHIFT) - 1;
        let (mut a, mut b) = (
            x.scene.quads[(x.entry & mask) as usize],
            y.scene.quads[(y.entry & mask) as usize],
        );
        if !clipped::plain(&a) || !clipped::plain(&b) {
            return false;
        }
        b.bounds = at(b.bounds, self.d);
        self.cut(&mut a.content_mask);
        self.shift(&mut b.content_mask);
        let visible = |q: &Quad| q.bounds.intersect(&q.content_mask.bounds);
        if !covers(visible(&a), p) || !covers(visible(&b), p) {
            return false;
        }
        let mut same = b;
        (same.bounds, same.content_mask, same.order) = (a.bounds, a.content_mask, a.order);
        same == a
    }

    fn exact_pair(&self, x: Item, y: Item) -> bool {
        let mask = (1 << KIND_SHIFT) - 1;
        let (i, j) = ((x.entry & mask) as usize, (y.entry & mask) as usize);
        let (sa, sb): (&Scene, &Scene) = (x.scene, y.scene);
        let d = self.d;
        macro_rules! sprite {
            ($list:ident) => {{
                let (mut a, mut b) = (sa.$list[i], sb.$list[j]);
                place(&mut b.bounds, &mut b.transformation, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                b.order = a.order;
                a == b
            }};
        }
        match x.entry >> KIND_SHIFT {
            0 => {
                let (mut a, mut b) = (sa.shadows[i], sb.shadows[j]);
                b.bounds = at(b.bounds, d);
                b.element_bounds = at(b.element_bounds, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                b.order = a.order;
                a == b
            }
            1 => {
                let (mut a, mut b) = (sa.quads[i], sb.quads[j]);
                if !movable(&a) || !movable(&b) {
                    return false;
                }
                place(&mut b.bounds, &mut b.transformation, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                if a.is_analytic_gradient() || b.is_analytic_gradient() {
                    return sa.same_gradient_quad(&a, sb, &b);
                }
                b.order = a.order;
                a == b || clipped::same_visible(&a, &b)
            }
            2 => {
                let (a, b) = (&sa.paths[i], &sb.paths[j]);
                if a.color_mapping.is_some() || b.color_mapping.is_some() {
                    return false;
                }
                let (mut a, mut b) = (a.clone(), b.clone());
                b.bounds = at(b.bounds, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                for v in &mut a.vertices {
                    self.cut(&mut v.content_mask);
                }
                for v in &mut b.vertices {
                    v.xy_position = v.xy_position
                        + Point {
                            x: ScaledPixels(d.0),
                            y: ScaledPixels(d.1),
                        };
                    self.shift(&mut v.content_mask);
                }
                same_path(&a, &b)
            }
            3 => {
                let (mut a, mut b) = (sa.underlines[i], sb.underlines[j]);
                b.bounds = at(b.bounds, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                b.order = a.order;
                a == b
            }
            4 => sprite!(monochrome_sprites),
            5 => sprite!(subpixel_sprites),
            6 => sprite!(polychrome_sprites),
            // A group's image or a backdrop: its own scene would have to move.
            _ => false,
        }
    }
}
