//! KaminIDE patch: a primitive of the new frame against one of the last frame
//! moved by `d` (`plan_shift`), both cut by the moved window `W`.
//!
//! A mask clips at pixel centres, so within a cell only the mask's part in
//! the cell decides pixels. A pair is compared once: a pair equal but for its
//! masks (the clip of a scroller moved against its stationary clip) keeps
//! both cut masks and is equal in each cell where the masks cut by the cell
//! are equal, so only the cells where the clips really part differ.

use super::super::{Area, KIND_SHIFT, clipped, same_path};
use super::Item;
use crate::color::BackgroundTag;
use crate::scene::{Quad, Scene};
use crate::{Bounds, ContentMask, Point, ScaledPixels, TransformationMatrix, point, size};
use collections::FxHashMap;

/// What a pair of primitives paints, compared once for all cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Pair {
    Same,
    Differ,
    /// Equal but for the masks: the new one and the moved old one, cut by `W`.
    Masks(Bounds<ScaledPixels>, Bounds<ScaledPixels>),
    /// Plain fills whose edges differ: compared again per cell, where an edge
    /// beyond the cell's part of both masks paints nothing.
    Quads,
}

/// The comparison of one plan: the move, `W` and the results by item pair.
pub(super) struct Moved<'g> {
    pub(super) d: (f32, f32),
    pub(super) w: Area,
    pub(super) exact: FxHashMap<(u32, u32), Pair>,
    /// Groups of the new frame matched under this move: the old group plus
    /// one (0: none), by group (`groups::match_moved`).
    pub(super) groups: &'g [u32],
    /// The scenes whose group composites' labels `groups` numbers (new, old):
    /// the frames, or two group scenes (their labels number the frames' groups).
    pub(super) labels: Option<(&'g Scene, &'g Scene)>,
}

pub(super) fn at(b: Bounds<ScaledPixels>, d: (f32, f32)) -> Bounds<ScaledPixels> {
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
/// relative to them (solid, gradients, their dither), not to the window.
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

/// The integer rectangle `p` (x0, y0, x1, y1) as bounds.
fn rect(p: [u32; 4]) -> Bounds<ScaledPixels> {
    let [x0, y0, x1, y1] = p.map(|v| v as f32);
    Bounds {
        origin: point(ScaledPixels(x0), ScaledPixels(y0)),
        size: size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)),
    }
}

/// `b` cut by `w`; a mask that lets no pixel through is equal whatever its place.
fn within(b: Bounds<ScaledPixels>, w: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
    let b = b.intersect(&w);
    if b.size.width.0 <= 0.0 || b.size.height.0 <= 0.0 {
        Bounds::default()
    } else {
        b
    }
}

/// `a == b` once `b` has `a`'s mask: equal but for the masks, or different.
fn masks<T: PartialEq>(
    a: &T,
    mut b: T,
    mask: fn(&mut T) -> &mut ContentMask<ScaledPixels>,
    ma: Bounds<ScaledPixels>,
) -> Pair {
    let mb = std::mem::replace(&mut mask(&mut b).bounds, ma);
    if *a == b {
        Pair::Masks(ma, mb)
    } else {
        Pair::Differ
    }
}

impl Moved<'_> {
    fn window(&self) -> Bounds<ScaledPixels> {
        let w = self.w;
        Bounds {
            origin: point(ScaledPixels(w[0]), ScaledPixels(w[1])),
            size: size(ScaledPixels(w[2] - w[0]), ScaledPixels(w[3] - w[1])),
        }
    }

    fn cut(&self, m: &mut ContentMask<ScaledPixels>) {
        m.bounds = within(m.bounds, self.window());
    }

    fn shift(&self, m: &mut ContentMask<ScaledPixels>) {
        m.bounds = at(m.bounds, self.d);
        self.cut(m);
    }

    /// Item `x` (new frame) paints within cell part `p` what item `y` (last
    /// frame) paints there once moved.
    pub(super) fn same(&mut self, x: (u32, Item), y: (u32, Item), p: [u32; 4]) -> bool {
        let pair = match self.exact.get(&(x.0, y.0)) {
            Some(&known) => known,
            None => {
                let known = self.exact_pair(x.1, y.1);
                self.exact.insert((x.0, y.0), known);
                known
            }
        };
        let equal = match pair {
            Pair::Same => true,
            Pair::Differ => false,
            Pair::Masks(a, b) => within(a, rect(p)) == within(b, rect(p)),
            Pair::Quads => self.quads_within(x.1, y.1, p),
        };
        equal || (x.1.entry >> KIND_SHIFT == 1 && self.same_fill(x.1, y.1, p))
    }

    /// Two plain fills within cell part `p`: an edge beyond the cell's part
    /// of both masks paints nothing there.
    fn quads_within(&self, x: Item, y: Item, p: [u32; 4]) -> bool {
        let mask = (1 << KIND_SHIFT) - 1;
        let (mut a, mut b) = (
            x.scene.quads[(x.entry & mask) as usize],
            y.scene.quads[(y.entry & mask) as usize],
        );
        place(&mut b.bounds, &mut b.transformation, self.d);
        self.cut(&mut a.content_mask);
        self.shift(&mut b.content_mask);
        a.content_mask.bounds = within(a.content_mask.bounds, rect(p));
        b.content_mask.bounds = within(b.content_mask.bounds, rect(p));
        b.order = a.order;
        a == b || clipped::same_visible(&a, &b)
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

    /// The pair's result for every cell; a moved clip that parts the pair's
    /// masks leaves it `Masks`, decided per cell.
    pub(super) fn exact_pair(&self, x: Item, y: Item) -> Pair {
        let verdict = |same: bool| if same { Pair::Same } else { Pair::Differ };
        let mask = (1 << KIND_SHIFT) - 1;
        let (i, j) = ((x.entry & mask) as usize, (y.entry & mask) as usize);
        let (sa, sb): (&Scene, &Scene) = (x.scene, y.scene);
        let d = self.d;
        macro_rules! pair {
            ($a:ident, $b:ident) => {{
                $b.order = $a.order;
                if $a == $b {
                    Pair::Same
                } else {
                    masks(&$a, $b, |p| &mut p.content_mask, $a.content_mask.bounds)
                }
            }};
        }
        macro_rules! sprite {
            ($list:ident) => {{
                let (mut a, mut b) = (sa.$list[i], sb.$list[j]);
                place(&mut b.bounds, &mut b.transformation, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                pair!(a, b)
            }};
        }
        match x.entry >> KIND_SHIFT {
            0 => {
                let (mut a, mut b) = (sa.shadows[i], sb.shadows[j]);
                b.bounds = at(b.bounds, d);
                b.element_bounds = at(b.element_bounds, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                pair!(a, b)
            }
            1 => {
                let (mut a, mut b) = (sa.quads[i], sb.quads[j]);
                if !movable(&a) || !movable(&b) {
                    return Pair::Differ;
                }
                place(&mut b.bounds, &mut b.transformation, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                if a.is_analytic_gradient() || b.is_analytic_gradient() {
                    return verdict(sa.same_gradient_quad(&a, sb, &b));
                }
                b.order = a.order;
                if a == b || clipped::same_visible(&a, &b) {
                    return Pair::Same;
                }
                match pair!(a, b) {
                    // Plain fills whose edges differ may still match per cell.
                    Pair::Differ if clipped::plain(&a) && clipped::plain(&b) => Pair::Quads,
                    other => other,
                }
            }
            2 => {
                let (a, b) = (&sa.paths[i], &sb.paths[j]);
                if a.color_mapping.is_some() || b.color_mapping.is_some() {
                    return Pair::Differ;
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
                // A mask per vertex: compared exactly, not per cell.
                verdict(same_path(&a, &b))
            }
            3 => {
                let (mut a, mut b) = (sa.underlines[i], sb.underlines[j]);
                b.bounds = at(b.bounds, d);
                self.cut(&mut a.content_mask);
                self.shift(&mut b.content_mask);
                pair!(a, b)
            }
            4 => sprite!(monochrome_sprites),
            5 => sprite!(subpixel_sprites),
            6 => sprite!(polychrome_sprites),
            // A group's composite: the same image when its group was matched
            // under this move. A backdrop sample reads the frame: never.
            _ => {
                let Some((ln, lo)) = self.labels else {
                    return Pair::Differ;
                };
                if !std::ptr::eq(sa, ln) || !std::ptr::eq(sb, lo) {
                    return Pair::Differ;
                }
                let (a, b) = (&sa.surfaces[i], &sb.surfaces[j]);
                verdict(
                    a.group > 0
                        && self.groups.get(a.group as usize - 1) == Some(&b.group)
                        && super::groups::same_surface_moved(
                            a,
                            b,
                            d,
                            |m| self.cut(m),
                            |m| self.shift(m),
                        ),
                )
            }
        }
    }
}
