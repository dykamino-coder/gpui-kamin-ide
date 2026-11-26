//! KaminIDE patch: the convex frame shape a group composite can colour.
//!
//! A tile pass (`group_tiles`) lists a composite in every tile its region
//! meets, and every pixel of a listed tile runs the composite's steps. For a
//! plane of a 3D context the region is the bounding box of its image, so a
//! face turned edge-on listed a box of tiles it barely crosses: on hzDAF the
//! pixel steps of a frame were several times the pixels the faces cover.
//! Where the composite colours is bounded more tightly by convex shapes:
//! the frame points whose raster point under the plane's homography lies in
//! its raster rectangle, in front of the eye (five half-planes: convex, and
//! bounded also where the rectangle reaches past the horizon), and its clip
//! polygon (a BSP piece or the near-clipped face) when that is convex. A tile
//! is listed only where those shapes meet it, as vello's coarse rasterizer
//! bins a path by the tiles its edges cross, not by its bounding box.

use super::BlurQuad;

/// Distance (px) a shape is grown by: the shader maps and rounds every
/// pixel in f32 and an antialiased polygon edge colours half a pixel out.
const MARGIN: f32 = 2.0;

/// Most edges of a shape: the five bounds of a plane's image and an 8-gon clip.
const MAX_EDGES: usize = 13;

/// Half-planes `a·x + b·y + c <= 0` (unit normals pointing out) whose
/// intersection, grown by `MARGIN`, holds every pixel the composite colours.
#[derive(Clone, Copy, Debug)]
pub(super) struct CoverShape {
    edges: [[f32; 3]; MAX_EDGES],
    len: usize,
}

impl CoverShape {
    /// The shape of composite `q` on a `vw` x `vh` target; `None` when nothing
    /// tighter than its region is known (no plane image and no convex clip).
    pub(super) fn of(q: &BlurQuad, vw: f32, vh: f32) -> Option<Self> {
        let mut shape = CoverShape {
            edges: [[0.0; 3]; MAX_EDGES],
            len: 0,
        };
        if q.proj[2][3] > 0.5 {
            // The shader samples only frame points p in front of the eye
            // (w = m2·p > 0) whose raster point (m0·p, m1·p) / w lies within
            // the target and the image rectangle (`group_source_with`). Each
            // bound is linear in p once multiplied by w > 0, so the set is
            // the intersection of five frame half-planes, bounded even where
            // the rectangle reaches behind the eye and its corners do not map.
            let s = q.source_rect;
            let rect = [s[0].max(0.0), s[1].max(0.0), (s[0] + s[2]).min(vw), (s[1] + s[3]).min(vh)];
            if !(rect[0] < rect[2] && rect[1] < rect[3]) {
                shape.add_half_plane([0.0, 0.0, 1.0]);
                return Some(shape);
            }
            let [m0, m1, m2] = [0, 1, 2].map(|r| [q.proj[r][0], q.proj[r][1], q.proj[r][2]]);
            // g·p >= 0 inside, for g = w, x - x0 w, y - y0 w, x1 w - x, y1 w - y.
            let side = |a: [f32; 3], k: f32, b: [f32; 3]| [0, 1, 2].map(|i| a[i] - k * b[i]);
            let neg = |g: [f32; 3]| g.map(|v| -v);
            for g in [
                m2,
                side(m0, rect[0], m2),
                side(m1, rect[1], m2),
                neg(side(m0, rect[2], m2)),
                neg(side(m1, rect[3], m2)),
            ] {
                shape.add_half_plane(neg(g));
            }
        }
        let count = (q.poly_count & 15) as usize;
        if q.poly_count & 16 == 0 && (3..=8).contains(&count) {
            let points: Vec<[f32; 2]> = (0..count)
                .map(|i| [q.poly[i / 2][(i % 2) * 2], q.poly[i / 2][(i % 2) * 2 + 1]])
                .collect();
            shape.add_convex(&points);
        }
        (shape.len > 0).then_some(shape)
    }

    /// Add the half-plane `e·(x, y, 1) <= 0`, its normal made a unit one. A
    /// line at infinity (no normal) holds everywhere or nowhere: the first
    /// adds nothing, the second an edge no point meets.
    fn add_half_plane(&mut self, e: [f32; 3]) {
        if self.len == MAX_EDGES || !e.iter().all(|v| v.is_finite()) {
            return;
        }
        let len = (e[0] * e[0] + e[1] * e[1]).sqrt();
        self.edges[self.len] = if len > f32::EPSILON * e[2].abs() {
            e.map(|v| v / len)
        } else if e[2] > 0.0 {
            [0.0, 0.0, f32::MAX]
        } else {
            return;
        };
        self.len += 1;
    }

    /// Add the edges of polygon `points` when it is convex and finite; a
    /// concave or degenerate one adds nothing (the region bounds it).
    fn add_convex(&mut self, points: &[[f32; 2]]) {
        let n = points.len();
        if n < 3 || self.len + n > MAX_EDGES || !points.iter().flatten().all(|v| v.is_finite()) {
            return;
        }
        let cross = |i: usize| {
            let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
            (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
        };
        let turns: Vec<f32> = (0..n).map(cross).collect();
        let sign = if turns.iter().all(|&t| t >= 0.0) {
            1.0
        } else if turns.iter().all(|&t| t <= 0.0) {
            -1.0
        } else {
            return;
        };
        let mut edges = [[0.0f32; 3]; 8];
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            // Outward normal of a counter-clockwise (sign 1, y down: clockwise
            // on screen) edge a -> b.
            let (nx, ny) = (sign * (b[1] - a[1]), sign * (a[0] - b[0]));
            let len = (nx * nx + ny * ny).sqrt();
            if !(len > 1e-6) {
                return;
            }
            let (nx, ny) = (nx / len, ny / len);
            edges[i] = [nx, ny, -(nx * a[0] + ny * a[1])];
        }
        self.edges[self.len..self.len + n].copy_from_slice(&edges[..n]);
        self.len += n;
    }

    /// The x interval (lo, hi) the shape (grown by `MARGIN`) may reach
    /// within the band of rows `y0..y1`; `None` when it misses the band.
    /// Each edge bounds x for its most permissive row of the band, so the
    /// interval holds every point of the shape there (it may hold more).
    pub(super) fn columns(&self, y0: f32, y1: f32) -> Option<(f32, f32)> {
        let (mut lo, mut hi) = (f32::NEG_INFINITY, f32::INFINITY);
        for &[a, b, c] in &self.edges[..self.len] {
            let rest = MARGIN - c - (b * y0).min(b * y1);
            if a.abs() < 1e-6 {
                if rest < 0.0 {
                    return None;
                }
            } else if a > 0.0 {
                hi = hi.min(rest / a);
            } else {
                lo = lo.max(rest / a);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// Whether the shape (grown by `MARGIN`) may meet the rectangle `r`
    /// (x0, y0, x1, y1).
    #[cfg(test)]
    fn meets(&self, r: [f32; 4]) -> bool {
        self.columns(r[1], r[3]).is_some_and(|(lo, hi)| lo <= r[2] && hi >= r[0])
    }
}

#[cfg(test)]
mod tests {
    use super::{BlurQuad, CoverShape};

    #[test]
    fn a_turned_plane_meets_only_the_tiles_its_image_crosses() {
        let mut q = BlurQuad::default();
        // Frame to raster: x' = x - y, y' = y, so the raster square 0..100
        // lies in the frame on the parallelogram (0,0) (100,0) (200,100) (100,100).
        q.proj = [[1.0, -1.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]];
        q.source_rect = [0.0, 0.0, 100.0, 100.0];
        let s = CoverShape::of(&q, 800.0, 600.0).unwrap();
        assert!(s.meets([40.0, 0.0, 56.0, 16.0]));
        assert!(s.meets([150.0, 80.0, 166.0, 96.0]));
        // Inside the bounding box (0..200 x 0..100), outside the parallelogram.
        assert!(!s.meets([0.0, 80.0, 16.0, 96.0]));
        assert!(!s.meets([176.0, 0.0, 192.0, 16.0]));
        // Within the margin of an edge it is kept.
        assert!(s.meets([101.0, 0.0, 117.0, 1.0]));
    }

    #[test]
    fn a_convex_clip_bounds_a_flat_composite_and_a_concave_one_does_not() {
        let mut q = BlurQuad::default();
        q.poly_count = 3;
        q.poly[0] = [0.0, 0.0, 100.0, 0.0];
        q.poly[1] = [0.0, 100.0, 0.0, 0.0];
        let s = CoverShape::of(&q, 800.0, 600.0).unwrap();
        assert!(s.meets([0.0, 0.0, 16.0, 16.0]));
        assert!(!s.meets([80.0, 80.0, 96.0, 96.0]));
        // Inverted polygons colour outside: nothing tighter is known.
        q.poly_count = 3 | 16;
        assert!(CoverShape::of(&q, 800.0, 600.0).is_none());
        // A concave quadrilateral (an arrow head) is left to the region.
        q.poly_count = 4;
        q.poly[0] = [0.0, 0.0, 100.0, 50.0];
        q.poly[1] = [0.0, 100.0, 30.0, 50.0];
        assert!(CoverShape::of(&q, 800.0, 600.0).is_none());
    }

    #[test]
    fn without_a_plane_or_a_clip_nothing_is_known() {
        assert!(CoverShape::of(&BlurQuad::default(), 800.0, 600.0).is_none());
    }

    /// The raster rectangle reaches past the horizon (its corners at x = 100
    /// map behind the eye): the image is unbounded to the right but its
    /// other sides still bound it.
    #[test]
    fn a_plane_reaching_past_the_horizon_is_bounded_by_its_half_planes() {
        let mut q = BlurQuad::default();
        // Frame to raster: (x, y) / w with w = 0.02 x + 1, so raster x stays
        // below 50 for every frame x and raster y <= 50 is frame y <= x + 50.
        q.proj = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.02, 0.0, 1.0, 1.0]];
        q.source_rect = [0.0, 0.0, 100.0, 50.0];
        let s = CoverShape::of(&q, 800.0, 600.0).unwrap();
        assert!(s.meets([700.0, 0.0, 716.0, 16.0]));
        assert!(s.meets([0.0, 40.0, 16.0, 56.0]));
        assert!(!s.meets([0.0, 200.0, 16.0, 216.0]));
        assert!(!s.meets([-40.0, 0.0, -24.0, 16.0]));
    }

    #[test]
    fn a_plane_image_off_the_target_meets_nothing() {
        let mut q = BlurQuad::default();
        q.proj = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]];
        q.source_rect = [900.0, 0.0, 100.0, 50.0];
        let s = CoverShape::of(&q, 800.0, 600.0).unwrap();
        assert!(!s.meets([0.0, 0.0, 800.0, 600.0]));
    }
}
