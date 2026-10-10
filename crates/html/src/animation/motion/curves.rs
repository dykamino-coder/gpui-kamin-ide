//! Аппроксимация кривых motion path ломаной.

/// Квадратик в кубик (SVG 2 §PathDataQuadraticBezierCommands, «two thirds of
/// the way»): первая контрольная точка.
pub(super) fn quad_c1(p0: (f32, f32), q: (f32, f32)) -> (f32, f32) {
    (
        p0.0 + 2.0 / 3.0 * (q.0 - p0.0),
        p0.1 + 2.0 / 3.0 * (q.1 - p0.1),
    )
}

/// Квадратик в кубик: вторая контрольная точка.
pub(super) fn quad_c2(p1: (f32, f32), q: (f32, f32)) -> (f32, f32) {
    (
        p1.0 + 2.0 / 3.0 * (q.0 - p1.0),
        p1.1 + 2.0 / 3.0 * (q.1 - p1.1),
    )
}

/// Кубик в точки (начало НЕ пишется — оно уже в ломаной).
pub(super) fn cubic_pts(
    p0: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    p1: (f32, f32),
    out: &mut Vec<(f32, f32)>,
) {
    const N: usize = 64;
    for k in 1..=N {
        let t = k as f32 / N as f32;
        let u = 1.0 - t;
        let b = |a: f32, b1: f32, b2: f32, d: f32| {
            u * u * u * a + 3.0 * u * u * t * b1 + 3.0 * u * t * t * b2 + t * t * t * d
        };
        out.push((b(p0.0, c1.0, c2.0, p1.0), b(p0.1, c1.1, c2.1, p1.1)));
    }
}

/// Дуга `A rx ry φ large sweep x y` в точки: перевод из КОНЦЕВОЙ записи в
/// центровую по SVG 2 §F.6.5 плюс раздутие недостаточных радиусов по §F.6.6.
pub(super) fn arc_pts(
    p0: (f32, f32),
    rx: f32,
    ry: f32,
    phi: f32,
    large: bool,
    sweep: bool,
    p1: (f32, f32),
    out: &mut Vec<(f32, f32)>,
) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    // §F.6.2: нулевой радиус или совпавшие концы — обычный отрезок.
    if rx == 0.0 || ry == 0.0 || ((p0.0 - p1.0).abs() + (p0.1 - p1.1).abs()) < 1e-9 {
        out.push(p1);
        return;
    }
    let (cs, sn) = (phi.cos(), phi.sin());
    let dx2 = (p0.0 - p1.0) / 2.0;
    let dy2 = (p0.1 - p1.1) / 2.0;
    let x1 = cs * dx2 + sn * dy2;
    let y1 = -sn * dx2 + cs * dy2;
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        rx *= lam.sqrt();
        ry *= lam.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        co = -co;
    }
    let cx1 = co * rx * y1 / ry;
    let cy1 = -co * ry * x1 / rx;
    let cx = cs * cx1 - sn * cy1 + (p0.0 + p1.0) / 2.0;
    let cy = sn * cx1 + cs * cy1 + (p0.1 + p1.1) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let d = (ux * vx + uy * vy) / ((ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt());
        let a = d.clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 { -a } else { a }
    };
    let (ux, uy) = ((x1 - cx1) / rx, (y1 - cy1) / ry);
    let (vx, vy) = ((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let t0 = ang(1.0, 0.0, ux, uy);
    let mut dt = ang(ux, uy, vx, vy);
    if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    }
    if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    const N: usize = 64;
    for k in 1..=N {
        let t = t0 + dt * (k as f32 / N as f32);
        let (xa, ya) = (rx * t.cos(), ry * t.sin());
        out.push((cx + cs * xa - sn * ya, cy + sn * xa + cs * ya));
    }
}
