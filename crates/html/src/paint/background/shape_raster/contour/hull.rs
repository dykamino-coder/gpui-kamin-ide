//! Ужатие радиусов противоположных вогнутых углов (css-borders-4
//! §corner-shape-constrain-radii, «opposite corner scale factor»): у каждого
//! угла — четырёхугольник-оболочка (начало, касательная в середине кривой,
//! конец); если оболочки диагональных углов пересекаются, оба угла
//! ужимаются от своей внешней вершины, пока не перестанут (бинарный поиск
//! с шагом 0.05, как `SolveOptimalHullScale` в Blink).

use super::corner::{Corner, half_corner};
use super::{P, add, scale, sub};

fn hull(c: &Corner) -> [P; 4] {
    let h = half_corner(c.curvature);
    let mid = add(c.center(), add(scale(c.v1(), h), scale(c.v4(), h)));
    let d = sub(mid, c.outer());
    let tangent = add(mid, (-d.1, d.0));
    let i1 = super::aligned::line_intersection(mid, tangent, c.start(), c.center())
        .unwrap_or(c.center());
    let i2 =
        super::aligned::line_intersection(mid, tangent, c.end(), c.center()).unwrap_or(c.center());
    [c.start(), i1, i2, c.end()]
}

fn bbox(q: &[P; 4]) -> (f32, f32, f32, f32) {
    q.iter()
        .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |b, p| {
            (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
        })
}

/// Пересечение выпуклых четырёхугольников (теорема о разделяющей оси).
fn intersects(a: &[P; 4], b: &[P; 4]) -> bool {
    for q in [a, b] {
        for i in 0..4 {
            let e = sub(q[(i + 1) % 4], q[i]);
            let n = (-e.1, e.0);
            if n == (0.0, 0.0) {
                continue;
            }
            let proj = |r: &[P; 4]| {
                r.iter().fold((f32::MAX, f32::MIN), |m, p| {
                    let v = n.0 * p.0 + n.1 * p.1;
                    (m.0.min(v), m.1.max(v))
                })
            };
            let (pa, pb) = (proj(a), proj(b));
            if pa.1 <= pb.0 || pb.1 <= pa.0 {
                return false;
            }
        }
    }
    true
}

fn scaled(q: &[P; 4], o: P, k: f32) -> [P; 4] {
    q.map(|p| add(o, scale(sub(p, o), k)))
}

fn factor(a: &Corner, b: &Corner) -> f32 {
    let (ba, bb) = (
        bbox(&[a.start(), a.outer(), a.end(), a.center()]),
        bbox(&[b.start(), b.outer(), b.end(), b.center()]),
    );
    if ba.2 <= bb.0 || bb.2 <= ba.0 || ba.3 <= bb.1 || bb.3 <= ba.1 {
        return 1.0;
    }
    let (ha, hb) = (hull(a), hull(b));
    if !intersects(&ha, &hb) {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    while hi - lo > 0.05 {
        let mid = (lo + hi) / 2.0;
        if intersects(&scaled(&ha, a.outer(), mid), &scaled(&hb, b.outer(), mid)) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// Множитель радиусов для углов TR, BR, BL, TL; 1 — если все выпуклые.
pub(crate) fn opposite_corner_scale(c: &[Corner; 4]) -> f32 {
    if c.iter().all(|c| c.curvature >= 1.0) {
        return 1.0;
    }
    factor(&c[3], &c[1]).min(factor(&c[2], &c[0])).min(1.0)
}
