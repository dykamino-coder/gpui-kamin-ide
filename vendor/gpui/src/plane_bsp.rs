//! KaminIDE patch: BSP ordering of the planes of one 3D rendering context.
//!
//! css-transforms-2 §3d-transform-rendering: planes of a context render by
//! depth, intersecting planes are cut. Same scheme as Chromium viz
//! (`components/viz/service/display/bsp_tree.cc`, `draw_polygon.cc`): the
//! first polygon of a list becomes the node, every other polygon is split by
//! its plane into front/back halves, and the tree is walked back to front
//! relative to a viewer at z = +∞ (CSS: positive z points to the viewer).
//! Points are device pixels after the perspective divide.

/// Polygons closer than this to a splitting plane count as lying on it.
const SPLIT_EPS: f32 = 0.05;

/// A convex polygon of one plane, tagged with its owner and document order.
#[derive(Clone, Debug)]
pub(crate) struct PlanePoly {
    pub points: Vec<[f32; 3]>,
    /// Edge `i` (point `i` → `i + 1`) was made by a BSP cut, not the box.
    pub cut: Vec<bool>,
    pub owner: usize,
    normal: [f32; 3],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn newell_raw(points: &[[f32; 3]]) -> [f32; 3] {
    let mut n = [0.0f32; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// Newell's normal of a polygon, normalised; `[0, 0, 1]` when degenerate.
fn newell(points: &[[f32; 3]]) -> [f32; 3] {
    let mut n = [0.0f32; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let len = dot(n, n).sqrt();
    if len < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

impl PlanePoly {
    pub fn new(points: Vec<[f32; 3]>, owner: usize) -> Self {
        let normal = newell(&points);
        PlanePoly {
            cut: vec![false; points.len()],
            points,
            owner,
            normal,
        }
    }

    /// Zero projected area (edge-on to the eye): invisible, and no plane to
    /// split by (cutting by it would only requantize the others' pixels).
    pub fn degenerate(&self) -> bool {
        newell_raw(&self.points)[2].abs() < 0.1
    }

    fn distance(&self, p: [f32; 3]) -> f32 {
        dot(self.normal, sub(p, self.points[0]))
    }
}

enum Side {
    Front(PlanePoly),
    Back(PlanePoly),
    Coplanar(PlanePoly),
    Split(PlanePoly, PlanePoly),
}

/// Split `poly` by the plane of `by` (viz `DrawPolygon::SplitPolygon`).
fn classify(by: &PlanePoly, poly: PlanePoly) -> Side {
    let d: Vec<f32> = poly.points.iter().map(|p| by.distance(*p)).collect();
    let front = d.iter().any(|v| *v > SPLIT_EPS);
    let back = d.iter().any(|v| *v < -SPLIT_EPS);
    match (front, back) {
        (false, false) => return Side::Coplanar(poly),
        (true, false) => return Side::Front(poly),
        (false, true) => return Side::Back(poly),
        _ => {}
    }
    // (point, lies on the cutting plane, original edge it starts on)
    let (mut f, mut b) = (Vec::new(), Vec::new());
    let n = poly.points.len();
    for i in 0..n {
        let (p, q) = (poly.points[i], poly.points[(i + 1) % n]);
        let (dp, dq) = (d[i], d[(i + 1) % n]);
        let on = dp.abs() <= SPLIT_EPS;
        if dp >= -SPLIT_EPS {
            f.push((p, on, i));
        }
        if dp <= SPLIT_EPS {
            b.push((p, on, i));
        }
        if (dp > SPLIT_EPS && dq < -SPLIT_EPS) || (dp < -SPLIT_EPS && dq > SPLIT_EPS) {
            let t = dp / (dp - dq);
            let x = [
                p[0] + (q[0] - p[0]) * t,
                p[1] + (q[1] - p[1]) * t,
                p[2] + (q[2] - p[2]) * t,
            ];
            f.push((x, true, i));
            b.push((x, true, i));
        }
    }
    let owner = poly.owner;
    let keep = |pts: Vec<([f32; 3], bool, usize)>| {
        let m = pts.len();
        let cut = (0..m)
            .map(|j| {
                let (a, c) = (pts[j], pts[(j + 1) % m]);
                (a.1 && c.1) || poly.cut[a.2]
            })
            .collect();
        PlanePoly {
            points: pts.iter().map(|p| p.0).collect(),
            cut,
            owner,
            normal: poly.normal,
        }
    };
    Side::Split(keep(f), keep(b))
}

/// Order polygons back to front; ties of coplanar polygons keep the list
/// order (document order). Returns the (possibly split) polygons.
pub(crate) fn back_to_front(mut list: Vec<PlanePoly>) -> Vec<PlanePoly> {
    let mut out = Vec::with_capacity(list.len());
    walk(&mut list, &mut out);
    out
}

fn walk(list: &mut Vec<PlanePoly>, out: &mut Vec<PlanePoly>) {
    if list.is_empty() {
        return;
    }
    let node = list.remove(0);
    let facing = node.normal[2] >= 0.0;
    let (mut front, mut back) = (Vec::new(), Vec::new());
    let mut coplanar = Vec::new();
    for poly in list.drain(..) {
        match classify(&node, poly) {
            Side::Front(p) => front.push(p),
            Side::Back(p) => back.push(p),
            Side::Coplanar(p) => coplanar.push(p),
            Side::Split(f, b) => {
                front.push(f);
                back.push(b);
            }
        }
    }
    // Viewer at +z: when the node faces it, its back half is farther.
    let (mut first, mut second) = if facing { (back, front) } else { (front, back) };
    walk(&mut first, out);
    out.push(node);
    out.extend(coplanar);
    walk(&mut second, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(z0: f32, z1: f32, owner: usize) -> PlanePoly {
        PlanePoly::new(
            vec![[0.0, 0.0, z0], [10.0, 0.0, z1], [10.0, 10.0, z1], [0.0, 10.0, z0]],
            owner,
        )
    }

    #[test]
    fn parallel_planes_sort_far_to_near() {
        let out = back_to_front(vec![quad(5.0, 5.0, 0), quad(-5.0, -5.0, 1)]);
        let owners: Vec<usize> = out.iter().map(|p| p.owner).collect();
        assert_eq!(owners, vec![1, 0]);
    }

    #[test]
    fn intersecting_planes_are_split() {
        let out = back_to_front(vec![quad(0.0, 0.0, 0), quad(-5.0, 5.0, 1)]);
        let owners: Vec<usize> = out.iter().map(|p| p.owner).collect();
        assert_eq!(owners, vec![1, 0, 1]);
    }
}
