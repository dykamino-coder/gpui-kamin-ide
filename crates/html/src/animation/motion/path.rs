//! Разбор и выборка SVG path для offset-distance.

use super::Poly;
use crate::animation::motion::curves::arc_pts;
use crate::animation::motion::curves::cubic_pts;
use crate::animation::motion::curves::quad_c1;
use crate::animation::motion::curves::quad_c2;

/// Разложить `d` в ломаную. Кроме прямых читаются кубики/квадратики (`C`,
/// `S`, `Q`, `T` — их даёт `shape()` и авторский `path()`) и дуги (`A` — их
/// даёт эквивалентный путь круга, эллипса и скруглённого прямоугольника).
/// Дробление фиксированное: 64 отрезка на сегмент — при радиусе 210px
/// (`offset-path-coord-box-001`) хорда отходит от дуги на 0.016px, а длина
/// занижается на четверть промилле; порог стенда — 0.5 % площади.
pub(super) fn flatten(d: &str) -> Option<Poly> {
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let mut closed = false;
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    // Отражение последней контрольной точки — для `S`/`T`
    // (SVG 2 §PathDataCubicBezierCommands: «the reflection of the second
    // control point on the previous command relative to the current point»).
    let (mut rfx, mut rfy) = (0.0f32, 0.0f32);
    let mut prev_curve = ' ';
    let toks = tokens(d);
    let mut i = 0usize;
    let mut cmd = ' ';
    while i < toks.len() {
        if let Some(ch) = toks[i].chars().next().filter(|c| c.is_ascii_alphabetic()) {
            cmd = ch;
            i += 1;
        }
        let num = |k: usize| -> f32 { toks.get(k).and_then(|t| t.parse().ok()).unwrap_or(0.0) };
        let rel = cmd.is_ascii_lowercase();
        // Снимок текущей точки: относительные команды отсчитываются от точки
        // НА НАЧАЛЕ команды (SVG 2 §PathData), а `cx`/`cy` по ходу разбора
        // переписываются — замыкание не должно держать их взаймы.
        let (bx, by) = (cx, cy);
        let ax = |v: f32| if rel { bx + v } else { v };
        let ay = |v: f32| if rel { by + v } else { v };
        let lower = cmd.to_ascii_lowercase();
        match lower {
            'm' => {
                cx = ax(num(i));
                cy = ay(num(i + 1));
                sx = cx;
                sy = cy;
                pts.push((cx, cy));
                i += 2;
                // Следующая пара после `m` — уже линия (SVG 2 §PathData).
                cmd = if rel { 'l' } else { 'L' };
            }
            'l' => {
                cx = ax(num(i));
                cy = ay(num(i + 1));
                pts.push((cx, cy));
                i += 2;
            }
            'h' => {
                cx = ax(num(i));
                pts.push((cx, cy));
                i += 1;
            }
            'v' => {
                cy = ay(num(i));
                pts.push((cx, cy));
                i += 1;
            }
            'c' | 's' | 'q' | 't' => {
                let p0 = (cx, cy);
                // Контрольные точки: у `S`/`T` первая — отражение прошлой,
                // у первой команды подряд отражать нечего, и она равна началу.
                let mirror = if matches!(prev_curve, 'c' | 's' | 'q' | 't') {
                    (2.0 * cx - rfx, 2.0 * cy - rfy)
                } else {
                    p0
                };
                let (c1, c2, end, n) = match lower {
                    'c' => (
                        (ax(num(i)), ay(num(i + 1))),
                        (ax(num(i + 2)), ay(num(i + 3))),
                        (ax(num(i + 4)), ay(num(i + 5))),
                        6,
                    ),
                    's' => (
                        mirror,
                        (ax(num(i)), ay(num(i + 1))),
                        (ax(num(i + 2)), ay(num(i + 3))),
                        4,
                    ),
                    'q' => {
                        let q = (ax(num(i)), ay(num(i + 1)));
                        let e = (ax(num(i + 2)), ay(num(i + 3)));
                        (quad_c1(p0, q), quad_c2(e, q), e, 4)
                    }
                    _ => {
                        let e = (ax(num(i)), ay(num(i + 1)));
                        (quad_c1(p0, mirror), quad_c2(e, mirror), e, 2)
                    }
                };
                cubic_pts(p0, c1, c2, end, &mut pts);
                // Отражать положено ВТОРУЮ контрольную точку кубика; у
                // квадратика — его единственную, поэтому её и запоминаем.
                (rfx, rfy) = if matches!(lower, 'q' | 't') {
                    if lower == 'q' {
                        (ax(num(i)), ay(num(i + 1)))
                    } else {
                        mirror
                    }
                } else {
                    c2
                };
                cx = end.0;
                cy = end.1;
                i += n;
            }
            'a' => {
                let p0 = (cx, cy);
                let (rx, ry) = (num(i), num(i + 1));
                let phi = num(i + 2).to_radians();
                let large = num(i + 3) != 0.0;
                let sweep = num(i + 4) != 0.0;
                let end = (ax(num(i + 5)), ay(num(i + 6)));
                arc_pts(p0, rx, ry, phi, large, sweep, end, &mut pts);
                cx = end.0;
                cy = end.1;
                i += 7;
            }
            'z' => {
                pts.push((sx, sy));
                cx = sx;
                cy = sy;
                closed = true;
                i += 1;
            }
            _ => break,
        }
        prev_curve = lower;
    }
    (!pts.is_empty()).then_some(Poly { pts, closed })
}

pub(super) fn tokens(d: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else if ch == '-' && !cur.is_empty() && !cur.ends_with('e') {
            out.push(std::mem::take(&mut cur));
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub(super) fn length(p: &[(f32, f32)]) -> f32 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

pub(super) fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Точка и угол касательной на расстоянии `s` от начала.
///
/// Вырожденный путь (`m 120 0 h 0 v 0 z`) даёт одну точку и нулевой угол —
/// именно этого ждут `offset-distance-004..006`.
pub(super) fn sample(p: &[(f32, f32)], s: f32) -> ((f32, f32), f32) {
    if p.len() < 2 {
        return (*p.first().unwrap_or(&(0.0, 0.0)), 0.0);
    }
    let mut left = s;
    for w in p.windows(2) {
        let seg = dist(w[0], w[1]);
        if seg <= 0.0 {
            continue;
        }
        if left <= seg {
            let t = left / seg;
            let pt = (
                w[0].0 + (w[1].0 - w[0].0) * t,
                w[0].1 + (w[1].1 - w[0].1) * t,
            );
            return (pt, (w[1].1 - w[0].1).atan2(w[1].0 - w[0].0));
        }
        left -= seg;
    }
    let last = p[p.len() - 1];
    let prev = p[p.len() - 2];
    (last, (last.1 - prev.1).atan2(last.0 - prev.0))
}
