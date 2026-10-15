//! Угол ВНУТРЕННЕГО контура, выровненный по внешнему (css-borders-4
//! §contour-path, «border-aligned corner clip-out path»), — перенос Blink
//! `Corner::AlignedToOrigin` и `CornerClipOutPath`: кривая угла сдвигается
//! по нормали на толщины сторон, а концы продлеваются «митрами» до сторон
//! внутреннего прямоугольника. Результат — цепочка от митры начала через
//! кривую до митры конца; из прямоугольника она вырезается.

use super::corner::{Corner, ROUND, add_curved_corner, len};
use super::{P, Seg, add, scale, sub};

fn norm(v: P) -> P {
    let l = len(v);
    if l == 0.0 { v } else { scale(v, 1.0 / l) }
}
fn cross(a: P, b: P) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
fn dot(a: P, b: P) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

/// Пересечение бесконечных прямых (a0, a1) и (b0, b1); параллельные — None.
pub(crate) fn line_intersection(a0: P, a1: P, b0: P, b1: P) -> Option<P> {
    let (a, b) = (sub(a1, a0), sub(b1, b0));
    let d = cross(a, b) as f64;
    if d.abs() < 1e-6 {
        return None;
    }
    let s = cross(sub(b0, a0), b) as f64 / d;
    Some(add(a0, scale(a, s as f32)))
}

fn segment_line_intersection(a0: P, a1: P, b0: P, b1: P) -> Option<P> {
    let (a, b) = (sub(a1, a0), sub(b1, b0));
    let d = cross(a, b) as f64;
    if d.abs() < 1e-6 {
        return None;
    }
    let off = sub(b0, a0);
    let sa = cross(off, b) as f64 / d;
    if (0.0..=1.0).contains(&sa) {
        return Some(add(a0, scale(a, sa as f32)));
    }
    let sb = cross(off, a) as f64 / d;
    if (0.0..=1.0).contains(&sb) {
        return Some(add(b0, scale(b, sb as f32)));
    }
    None
}

/// Толщины у начала и конца угла: за радиусом — толщина стороны (радиус
/// внутреннего угла зажат нулём), иначе — разница радиусов.
fn insets(origin: &Corner, target: &Corner, edge_start: f32, edge_end: f32) -> (f32, f32) {
    let (sr, er) = (len(origin.v2()), len(origin.v3()));
    let start = if edge_start >= 0.0 && len(target.v2()) == 0.0 {
        edge_start
    } else {
        sr - len(target.v2())
    };
    let end = if edge_end >= 0.0 && len(target.v3()) == 0.0 {
        edge_end
    } else {
        er - len(target.v3())
    };
    (start, end)
}

/// `Corner::AlignedToOrigin`: кривая внешнего угла, сдвинутая внутрь.
pub(crate) fn aligned_to_origin(
    target: &Corner,
    origin: &Corner,
    edge_start: f32,
    edge_end: f32,
) -> Corner {
    if origin.is_empty() || target == origin {
        return *target;
    }
    let (sr, er) = (len(origin.v2()), len(origin.v3()));
    let (si, ei) = insets(origin, target, edge_start, edge_end);
    let param = origin.curvature.log2();
    let exponent = param.abs().exp2();
    let convex_half = 0.5f32.powf(1.0 / exponent);
    let half = if param < 0.0 {
        1.0 - convex_half
    } else {
        convex_half
    };
    let sqrt2 = std::f32::consts::SQRT_2;
    let cpx = (half / (sqrt2 - 1.0) - 1.0 / sqrt2).clamp(0.0, 1.0);
    let diff = (ei - si).clamp(-sr, er);
    let (mut scx, mut ecx) = (cpx, cpx);
    if diff != 0.0 {
        let delta = (sr * sr + er * er - diff * diff).sqrt();
        let nx = er * diff + sr * delta;
        let ny = -sr * diff + er * delta;
        let bevel_cpx = sr * ny / (sr * ny + er * nx);
        scx = if param < 0.0 {
            bevel_cpx * (2.0 * cpx)
        } else {
            1.0 - (1.0 - bevel_cpx) * (2.0 * (1.0 - cpx))
        };
        ecx = 2.0 * cpx - scx;
    }
    let us = norm(((1.0 - scx) * sr, scx * er));
    let ue = norm((ecx * sr, (1.0 - ecx) * er));
    let (n3, n2) = (norm(origin.v3()), norm(origin.v2()));
    let sn = add(scale(n3, us.0), scale(n2, us.1));
    let en = add(scale(n3, ue.0), scale(n2, ue.1));
    let o = target.outer();
    let original_outer = sub(sub(o, scale(n3, ei)), scale(n2, si));
    let mut a_start = add(add(original_outer, scale(n3, er)), scale(sn, si));
    let mut a_end = add(add(original_outer, scale(n2, sr)), scale(en, ei));
    let st = (-sn.1, sn.0);
    let et = (-en.1, en.0);
    if param >= 0.0 && si < 0.0 {
        a_start = line_intersection(a_start, sub(a_start, st), add(o, n3), o).unwrap_or(a_start);
    }
    if param >= 0.0 && ei < 0.0 {
        a_end = line_intersection(a_end, add(a_end, et), add(o, n2), o).unwrap_or(a_end);
    }
    let h = dot(sub(a_end, a_start), n2);
    let a_outer = sub(a_end, scale(n2, h));
    let a_center = add(a_start, scale(n2, h));
    if param <= -1.0 || param >= 0.0 {
        return Corner::of_vertices([a_start, a_outer, a_end, a_center], origin.curvature);
    }
    let ti = line_intersection(a_start, sub(a_start, st), a_end, add(a_end, et)).unwrap_or(a_start);
    Corner::of_vertices([a_start, ti, a_end, add(a_start, sub(a_end, ti))], ROUND)
}

/// `CornerClipOutPath` без замыкающей вершины: цепочка от митры начала до
/// митры конца. None — угол не вырезается (прямой или без радиуса).
pub(crate) fn clip_out(
    origin: &Corner,
    target: &Corner,
    edge_start: f32,
    edge_end: f32,
) -> Option<(P, Vec<Seg>)> {
    let (sr, er) = (len(origin.v2()), len(origin.v3()));
    if sr == 0.0 || er == 0.0 || origin.is_straight() {
        return None;
    }
    let (si, ei) = insets(origin, target, edge_start, edge_end);
    let param = origin.curvature.log2();
    let diff = (ei - si).clamp(-sr, er);
    if param <= 0.0 && (diff == -sr || diff == er) {
        return None;
    }
    let a = aligned_to_origin(target, origin, edge_start, edge_end);
    let (n3, n2) = (norm(origin.v3()), norm(origin.v2()));
    let o = target.outer();
    let original_outer = sub(sub(o, scale(n3, ei)), scale(n2, si));
    let original_start = add(original_outer, scale(n3, er));
    let original_end = add(original_outer, scale(n2, sr));
    let (mut ms, mut me) = (a.start(), a.end());
    if param < 0.0 && si < 0.0 {
        let n = norm(scale(sub(a.start(), original_start), 1.0 / si));
        let t = (-n.1, n.0);
        ms = line_intersection(a.start(), sub(a.start(), t), add(o, n3), o).unwrap_or(a.start());
    }
    if param < 0.0 && ei < 0.0 {
        let n = norm(scale(sub(a.end(), original_end), 1.0 / ei));
        let t = (-n.1, n.0);
        me = line_intersection(a.end(), add(a.end(), t), add(o, n2), o).unwrap_or(a.end());
    }
    let selfx = (param < 0.0 && si < 0.0 && ei < 0.0 && (-ei >= sr || -si >= er))
        .then(|| segment_line_intersection(ms, a.start(), me, a.end()))
        .flatten();
    let mut segs = Vec::new();
    if let Some(p) = selfx {
        segs.push(Seg::Line(p));
    } else if origin.is_notch() {
        segs.push(Seg::Line(a.center()));
    } else {
        add_curved_corner(&mut segs, &a);
    }
    segs.push(Seg::Line(me));
    Some((ms, segs))
}
