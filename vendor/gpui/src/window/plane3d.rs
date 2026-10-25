//! KaminIDE patch: planes of CSS 3D rendering contexts, painted through buffers.
//!
//! css-transforms-2 §3d-transform-rendering: each plane is rendered flat and
//! then mapped by its accumulated 4×4 matrix. A plane's content is painted
//! into a group buffer under an affine "raster" transform (the affine part
//! of the plane's matrix, so pure 2D cases stay pixel-exact), and the buffer
//! is composited with the remaining homography: the pixel shader maps each
//! frame pixel back through H⁻¹ (perspective-correct sampling, like viz
//! drawing a textured quad with a full 4×4). Planes of one context are
//! ordered and split by a BSP tree (`crate::plane_bsp`) before compositing.

use super::*;
use crate::plane_bsp::{PlanePoly, back_to_front};
use crate::{PaintGroup, PaintSurface};

/// Near-plane threshold for `w` of homogeneous corners.
const W_EPS: f32 = 1e-3;

/// Pending planes of the innermost open 3D rendering context.
pub(crate) struct PlaneContext {
    pending: Vec<Option<PlaneRecord>>,
    content_mask: ContentMask<ScaledPixels>,
}

struct PlaneRecord {
    scene: Scene,
    projection: Option<[f32; 9]>,
    corners: [[f32; 4]; 4],
}

/// How a plane is painted and composited (all in device pixels of the
/// coordinate space the caller paints in).
pub struct PlaneSpec {
    /// Absolute transform the plane's content is painted under.
    pub raster: TransformationMatrix,
    /// Frame point → raster point (3×3 row-major, homogeneous); `None` when
    /// the plane's map is the raster transform itself.
    pub projection: Option<[f32; 9]>,
    /// The plane's full 4x4 (row-major): local device point (z = 0) to the
    /// context space.
    pub full: [[f32; 4]; 4],
    /// Affine transform active outside the context (applied after `full`).
    pub outer: TransformationMatrix,
    /// The plane's box in local device points (x0, y0, x1, y1); grown by the
    /// painted content that overflows it when the plane closes.
    pub rect: (f32, f32, f32, f32),
    /// Child masks follow the raster transform (`with_transformation_masked`).
    pub masked: bool,
}

/// Corners of `rect` mapped by `full` and the outer affine: (x, y, z, w).
fn plane_corners(
    full: &[[f32; 4]; 4],
    outer: TransformationMatrix,
    rect: (f32, f32, f32, f32),
) -> [[f32; 4]; 4] {
    let (r, t) = (outer.rotation_scale, outer.translation);
    let (x0, y0, x1, y1) = rect;
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| {
        let row = |i: usize| full[i][0] * x + full[i][1] * y + full[i][3];
        let (px, py, pz, pw) = (row(0), row(1), row(2), row(3));
        [
            r[0][0] * px + r[0][1] * py + t[0] * pw,
            r[1][0] * px + r[1][1] * py + t[1] * pw,
            pz,
            pw,
        ]
    })
}

/// Bounds of everything a plane's scene painted, in the plane's local device
/// points (the space `PlaneSpec::rect` lives in): primitive bounds are mapped
/// by their own transformation and back through the raster map. A plane's
/// content may overflow its box (an empty `div` holding an absolutely
/// positioned child); like a compositor layer, the plane covers its ink.
fn ink_bounds(scene: &Scene, raster: TransformationMatrix) -> Option<(f32, f32, f32, f32)> {
    let [[a, b], [c, d]] = raster.rotation_scale;
    let det = a * d - b * c;
    if det.abs() < 1e-9 {
        return None;
    }
    let (t0, t1) = (raster.translation[0], raster.translation[1]);
    let inverse = |x: f32, y: f32| {
        let (x, y) = (x - t0, y - t1);
        ((d * x - b * y) / det, (-c * x + a * y) / det)
    };
    let mut ink: Option<(f32, f32, f32, f32)> = None;
    let mut add = |bounds: &Bounds<ScaledPixels>, map: Option<TransformationMatrix>| {
        let (x0, y0) = (bounds.origin.x.0, bounds.origin.y.0);
        let (x1, y1) = (x0 + bounds.size.width.0, y0 + bounds.size.height.0);
        for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
            let (x, y) = match map {
                Some(m) => {
                    let r = m.rotation_scale;
                    let (px, py) = (
                        r[0][0] * x + r[0][1] * y + m.translation[0],
                        r[1][0] * x + r[1][1] * y + m.translation[1],
                    );
                    inverse(px, py)
                }
                None => (x, y),
            };
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            ink = Some(match ink {
                Some((ax, ay, bx, by)) => (ax.min(x), ay.min(y), bx.max(x), by.max(y)),
                None => (x, y, x, y),
            });
        }
    };
    for q in &scene.quads {
        add(&q.bounds, Some(q.transformation));
    }
    for s in &scene.monochrome_sprites {
        add(&s.bounds, Some(s.transformation));
    }
    for s in &scene.subpixel_sprites {
        add(&s.bounds, Some(s.transformation));
    }
    for s in &scene.polychrome_sprites {
        add(&s.bounds, Some(s.transformation));
    }
    for s in &scene.shadows {
        add(&s.bounds, None);
    }
    for u in &scene.underlines {
        add(&u.bounds, None);
    }
    for p in &scene.paths {
        add(&p.bounds, None);
    }
    ink
}

/// Clip a homogeneous polygon to `w ≥ W_EPS` and divide; `None` if nothing
/// is in front of the eye. `clipped` tells whether a corner was cut away.
fn near_clip(corners: &[[f32; 4]; 4]) -> Option<(Vec<[f32; 3]>, bool)> {
    let mut out = Vec::new();
    let mut clipped = false;
    for i in 0..4 {
        let (p, q) = (corners[i], corners[(i + 1) % 4]);
        let (pin, qin) = (p[3] >= W_EPS, q[3] >= W_EPS);
        if pin {
            out.push(p);
        } else {
            clipped = true;
        }
        if pin != qin {
            let t = (W_EPS - p[3]) / (q[3] - p[3]);
            out.push([0, 1, 2, 3].map(|k| p[k] + (q[k] - p[k]) * t));
        }
    }
    if out.len() < 3 {
        return None;
    }
    let pts = out
        .iter()
        .map(|v| [v[0] / v[3], v[1] / v[3], v[2] / v[3]])
        .collect();
    Some((pts, clipped))
}

fn to_points(points: &[[f32; 3]]) -> Vec<Point<ScaledPixels>> {
    points
        .iter()
        .map(|p| point(ScaledPixels(p[0]), ScaledPixels(p[1])))
        .collect()
}

/// A BSP piece's polygon with its box edges pushed 1px outward: the content
/// is already antialiased along the box, only the cuts need the polygon's
/// coverage (otherwise the box edge would be faded twice).
fn grow_box_edges(poly: &PlanePoly) -> Vec<[f32; 3]> {
    let pts = &poly.points;
    let n = pts.len();
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    // Edge lines as (point, direction, outward offset).
    let lines: Vec<([f32; 2], [f32; 2])> = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if poly.cut[i] || len < 1e-4 {
                return ([a[0], a[1]], d);
            }
            // Outward normal of a counter-clockwise (area > 0) polygon.
            let nrm = [sign * d[1] / len, -sign * d[0] / len];
            ([a[0] + nrm[0], a[1] + nrm[1]], d)
        })
        .collect();
    (0..n)
        .map(|i| {
            let (p0, d0) = lines[(i + n - 1) % n];
            let (p1, d1) = lines[i];
            let den = d0[0] * d1[1] - d0[1] * d1[0];
            if den.abs() < 1e-6 {
                return [p1[0], p1[1], pts[i][2]];
            }
            let t = ((p1[0] - p0[0]) * d1[1] - (p1[1] - p0[1]) * d1[0]) / den;
            [p0[0] + d0[0] * t, p0[1] + d0[1] * t, pts[i][2]]
        })
        .collect()
}

impl Window {
    /// Paint `f` as one 3D rendering context: planes opened inside it
    /// (`paint_plane`) are depth-sorted and split when it closes.
    pub fn paint_plane_context<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_paint();
        let content_mask = self.content_mask().scale(self.scale_factor());
        self.plane_contexts.push(PlaneContext {
            pending: Vec::new(),
            content_mask,
        });
        let (result, ctx) = self.with_restore(None, |window, ctx| {
            *ctx = window.plane_contexts.pop();
        }, |window| window.with_paint_scope(|window| {
            crate::elements::paint_last::hoist_boundary(|| f(window))
        }));
        let Some(ctx) = ctx else { return result };
        let records: Vec<PlaneRecord> = ctx.pending.into_iter().flatten().collect();
        let mut polys = Vec::new();
        for (i, rec) in records.iter().enumerate() {
            if let Some((pts, _)) = near_clip(&rec.corners) {
                let poly = PlanePoly::new(pts, i);
                // Edge-on planes are invisible and must not split others.
                if !poly.degenerate() {
                    polys.push(poly);
                }
            }
        }
        let ordered = back_to_front(polys);
        let mut pieces = vec![0usize; records.len()];
        for p in &ordered {
            pieces[p.owner] += 1;
        }
        let mut ids = vec![None; records.len()];
        let mut inline: Vec<Option<Scene>> = (0..records.len()).map(|_| None).collect();
        let mut first = vec![0u32; records.len()];
        for (i, rec) in records.into_iter().enumerate() {
            if pieces[i] == 0 {
                continue;
            }
            let polygon = match near_clip(&rec.corners) {
                Some((pts, true)) => to_points(&pts),
                _ => Vec::new(),
            };
            // An affine plane that is neither cut nor clipped needs no
            // buffer: its primitives are replayed straight into the frame at
            // its place in the BSP order (byte-identical to painting it
            // directly; a buffer would requantize glyph coverage).
            if rec.projection.is_none() && pieces[i] == 1 && polygon.is_empty() {
                inline[i] = Some(self.adopt_plane_scene(rec.scene));
                continue;
            }
            let fragments = if pieces[i] > 1 {
                ordered
                    .iter()
                    .filter(|p| p.owner == i)
                    .map(|p| to_points(&grow_box_edges(p)))
                    .collect()
            } else {
                Vec::new()
            };
            ids[i] = Some(self.push_plane_group(rec.scene, rec.projection, polygon, fragments));
        }
        for p in &ordered {
            if let Some(scene) = inline[p.owner].take() {
                let ops = scene.paint_operations.len();
                self.next_frame.scene.replay(0..ops, &scene);
                continue;
            }
            let Some(id) = ids[p.owner] else { continue };
            let fragment = if pieces[p.owner] > 1 {
                first[p.owner] += 1;
                first[p.owner]
            } else {
                0
            };
            self.insert_plane_mark(id, fragment, ctx.content_mask.clone());
        }
        result
    }

    /// Paint `f` as one plane (see the module docs). Inside an open context
    /// the plane is composited when the context closes; otherwise at once.
    pub fn paint_plane<R>(&mut self, spec: PlaneSpec, f: impl FnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_paint();
        let slot = self.plane_contexts.last_mut().map(|c| {
            c.pending.push(None);
            c.pending.len() - 1
        });
        let depth = self.plane_contexts.len();
        let content_mask = self.content_mask().scale(self.scale_factor());
        let r = spec.raster.rotation_scale;
        let keeps_axes = (r[0][1].abs() < 1e-4 && r[1][0].abs() < 1e-4)
            || (r[0][0].abs() < 1e-4 && r[1][1].abs() < 1e-4);
        let map = (spec.masked && keeps_axes).then_some(spec.raster);
        let (result, mut scene) = self.with_isolated_scene(|window| {
            let state = (
                mem::replace(&mut window.transformation_stack, vec![spec.raster]),
                mem::replace(&mut window.mask_map, map),
                mem::take(&mut window.content_mask_stack),
            );
            window.with_restore(state, |window, state| {
                mem::swap(&mut window.transformation_stack, &mut state.0);
                mem::swap(&mut window.mask_map, &mut state.1);
                mem::swap(&mut window.content_mask_stack, &mut state.2);
            }, f).0
        });
        scene.finish();
        let mut rect = spec.rect;
        if let Some((ix0, iy0, ix1, iy1)) = ink_bounds(&scene, spec.raster) {
            rect = (
                rect.0.min(ix0),
                rect.1.min(iy0),
                rect.2.max(ix1),
                rect.3.max(iy1),
            );
        }
        let record = PlaneRecord {
            scene,
            projection: spec.projection,
            corners: plane_corners(&spec.full, spec.outer, rect),
        };
        match slot {
            Some(i) if self.plane_contexts.len() == depth => {
                if let Some(ctx) = self.plane_contexts.last_mut() {
                    ctx.pending[i] = Some(record);
                }
            }
            _ => {
                let Some((pts, clipped)) = near_clip(&record.corners) else {
                    return result;
                };
                let polygon = if clipped { to_points(&pts) } else { Vec::new() };
                let id = self.push_plane_group(record.scene, record.projection, polygon, Vec::new());
                self.insert_plane_mark(id, 0, content_mask);
            }
        }
        result
    }

    /// Prepare a plane scene for replay into the current one: its nested
    /// groups join the frame's list (ids shifted), hoisted backdrops go first.
    fn adopt_plane_scene(&mut self, mut inner: Scene) -> Scene {
        let hoisted = std::mem::take(&mut inner.hoisted_backdrops);
        let mut nested = std::mem::take(&mut inner.groups);
        let base = self.next_frame.scene.groups.len() as u32;
        if !nested.is_empty() {
            inner.shift_group_ids(base);
            for group in &mut nested {
                group.scene.shift_group_ids(base);
            }
            self.next_frame.scene.groups.append(&mut nested);
        }
        if self.next_frame.scene.in_group {
            self.next_frame.scene.hoisted_backdrops.extend(hoisted);
        } else {
            for surface in hoisted {
                self.next_frame.scene.insert_primitive(surface);
            }
        }
        inner
    }

    /// Move a finished plane scene into the frame's group list (nested
    /// groups first, as `paint_group` does); returns its group index.
    fn push_plane_group(
        &mut self,
        mut inner: Scene,
        projection: Option<[f32; 9]>,
        polygon: Vec<Point<ScaledPixels>>,
        fragments: Vec<Vec<Point<ScaledPixels>>>,
    ) -> u32 {
        let hoisted = std::mem::take(&mut inner.hoisted_backdrops);
        let mut nested = std::mem::take(&mut inner.groups);
        let base = self.next_frame.scene.groups.len() as u32;
        if !nested.is_empty() {
            inner.shift_group_ids(base);
            for group in &mut nested {
                group.scene.shift_group_ids(base);
            }
            self.next_frame.scene.groups.append(&mut nested);
        }
        let index = self.next_frame.scene.groups.len() as u32;
        let size = self.viewport_size.scale(self.scale_factor());
        self.next_frame.scene.groups.push(PaintGroup {
            scene: inner,
            bounds: Bounds::new(Point::default(), size),
            polygon,
            projection,
            fragments,
            ..Default::default()
        });
        if self.next_frame.scene.in_group {
            self.next_frame.scene.hoisted_backdrops.extend(hoisted);
        } else {
            for surface in hoisted {
                self.next_frame.scene.insert_primitive(surface);
            }
        }
        index
    }

    fn insert_plane_mark(&mut self, index: u32, fragment: u32, mask: ContentMask<ScaledPixels>) {
        self.next_frame.scene.insert_primitive(PaintSurface {
            order: 0,
            bounds: mask.bounds,
            content_mask: mask,
            corner_radii: Corners::default(),
            blur_radius: 0.0,
            group: index + 1,
            opacity: 1.0,
            color_matrix: None,
            fragment,
        });
    }
}
