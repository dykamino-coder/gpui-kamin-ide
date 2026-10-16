//! Контур коробки с фигурными углами (css-borders-4 §contour-path).
//!
//! Внешний контур — один путь по четырём углам (`draw_as_single_path` Blink),
//! внутренний — прямоугольник, сжатый на толщины сторон, из которого
//! вырезаны выровненные углы (`AddContouredRect` с `CornerClipOutPath`).
//! Прямолинейный контур (bevel/notch/square) отдаётся многоугольником —
//! его рисует тот же шейдер, что `clip-path: polygon()`; кривые идут
//! SVG-путём в тот же растр, что `clip-path: path()`.

mod aligned;
mod corner;
mod hull;
mod offset;
#[cfg(test)]
mod tests;

use corner::{Corner, NOTCH, ROUND, STRAIGHT, add_curved_corner};
pub use offset::offset;

pub(crate) type P = (f32, f32);

pub(crate) fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
pub(crate) fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
pub(crate) fn scale(a: P, k: f32) -> P {
    (a.0 * k, a.1 * k)
}

/// Отрезок пути от текущей точки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Line(P),
    Cubic(P, P, P),
}

/// Коробка с углами: прямоугольник `(x, y, w, h)`, радиусы tl/tr/br/bl
/// (уже ужатые, css-backgrounds-3 §5.5) и параметры K суперэллипса.
#[derive(Clone, Copy, Debug)]
pub struct Shaped {
    pub rect: (f32, f32, f32, f32),
    pub radii: [P; 4],
    pub k: [f32; 4],
}

/// Показатель угла: без радиуса — круглый (`EffectiveCurvature`), K ≥ 16 —
/// прямой, K ≤ −16 — выемка (`Superellipse::IsDegenerate`/`IsFullyConcave`).
fn curvature(k: f32, r: P) -> f32 {
    if r.0 <= 0.0 || r.1 <= 0.0 {
        ROUND
    } else if k >= 16.0 {
        STRAIGHT
    } else if k <= -16.0 {
        NOTCH
    } else {
        k.exp2()
    }
}

impl Shaped {
    /// Радиусы, ужатые так, чтобы оболочки диагональных вогнутых углов не
    /// пересекались (`opposite corner scale factor`).
    pub fn constrained(mut self) -> Self {
        let f = hull::opposite_corner_scale(&self.corners());
        if f < 1.0 {
            self.radii = self.radii.map(|(x, y)| (x * f, y * f));
        }
        self
    }

    /// Углы в порядке обхода TR, BR, BL, TL.
    fn corners(&self) -> [Corner; 4] {
        let (x, y, w, h) = self.rect;
        let [tl, tr, br, bl] = self.radii;
        let [ktl, ktr, kbr, kbl] = self.k;
        [
            Corner::of_rect((x + w - tr.0, y, tr.0, tr.1), 0, curvature(ktr, tr)),
            Corner::of_rect(
                (x + w - br.0, y + h - br.1, br.0, br.1),
                1,
                curvature(kbr, br),
            ),
            Corner::of_rect((x, y + h - bl.1, bl.0, bl.1), 2, curvature(kbl, bl)),
            Corner::of_rect((x, y, tl.0, tl.1), 3, curvature(ktl, tl)),
        ]
    }

    /// Прямоугольник, сжатый на толщины t/r/b/l, с радиусами за вычетом
    /// толщин (`FloatRoundedRect::Inset`).
    fn inset(&self, [t, r, b, l]: [f32; 4]) -> Shaped {
        let (x, y, w, h) = self.rect;
        let [tl, tr, br, bl] = self.radii;
        let s = |(rx, ry): P, dx: f32, dy: f32| ((rx - dx).max(0.0), (ry - dy).max(0.0));
        Shaped {
            rect: (x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0)),
            radii: [s(tl, l, t), s(tr, r, t), s(br, r, b), s(bl, l, b)],
            k: self.k,
        }
    }
}

/// Замкнутый контур: начальная точка и отрезки.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub start: P,
    pub segs: Vec<Seg>,
}

impl Path {
    /// Вершины, если контур прямолинейный; соседние совпадения убраны.
    pub fn polygon(&self) -> Option<Vec<P>> {
        let mut out = vec![self.start];
        for s in &self.segs {
            match s {
                Seg::Line(p) => out.push(*p),
                Seg::Cubic(..) => return None,
            }
        }
        dedup(&mut out);
        Some(out)
    }

    /// Запись `d` для SVG.
    pub fn svg_d(&self) -> String {
        let mut d = format!("M{} {}", self.start.0, self.start.1);
        for s in &self.segs {
            match s {
                Seg::Line(p) => d.push_str(&format!(" L{} {}", p.0, p.1)),
                Seg::Cubic(a, b, c) => d.push_str(&format!(
                    " C{} {} {} {} {} {}",
                    a.0, a.1, b.0, b.1, c.0, c.1
                )),
            }
        }
        d.push_str(" Z");
        d
    }
}

fn dedup(v: &mut Vec<P>) {
    v.dedup();
    while v.len() > 1 && v.first() == v.last() {
        v.pop();
    }
}

/// Внешний контур: путь по четырём углам.
pub fn outer(s: &Shaped) -> Path {
    let corners = s.corners();
    let mut segs = Vec::new();
    for c in &corners {
        add_curved_corner(&mut segs, c);
    }
    Path {
        start: corners[0].start(),
        segs,
    }
}

/// Внутренний контур при толщинах t/r/b/l.
pub enum Inner {
    /// Прямолинейный: многоугольник.
    Polygon(Vec<P>),
    /// С кривыми: прямоугольник и вырезаемые из него углы (замкнутые пути).
    Clipped {
        rect: (f32, f32, f32, f32),
        clips: Vec<Path>,
    },
}

pub fn inner(s: &Shaped, insets: [f32; 4]) -> Inner {
    let target = s.inset(insets);
    let (origin_c, target_c) = (s.corners(), target.corners());
    let [t, r, b, l] = insets;
    let edges = [(t, r), (r, b), (b, l), (l, t)];
    let mut clips = Vec::new();
    let mut linear = true;
    let mut chains: Vec<Option<(P, Vec<Seg>)>> = Vec::new();
    for i in 0..4 {
        let c = aligned::clip_out(&origin_c[i], &target_c[i], edges[i].0, edges[i].1);
        if let Some((ms, segs)) = &c {
            linear &= segs.iter().all(|s| matches!(s, Seg::Line(_)));
            let mut closed = segs.clone();
            closed.push(Seg::Line(target_c[i].outer()));
            clips.push(Path {
                start: *ms,
                segs: closed,
            });
        }
        chains.push(c);
    }
    if !linear {
        return Inner::Clipped {
            rect: target.rect,
            clips,
        };
    }
    let mut pts = Vec::new();
    for (i, chain) in chains.iter().enumerate() {
        match chain {
            None => pts.push(target_c[i].outer()),
            Some((ms, segs)) => {
                let mut line = vec![*ms];
                line.extend(segs.iter().filter_map(|s| match s {
                    Seg::Line(p) => Some(*p),
                    Seg::Cubic(..) => None,
                }));
                clip_polyline(&line, target.rect, &mut pts);
            }
        }
    }
    dedup(&mut pts);
    Inner::Polygon(pts)
}

/// Части ломаной внутри прямоугольника (Лян–Барски по каждому звену).
fn clip_polyline(line: &[P], (x, y, w, h): (f32, f32, f32, f32), out: &mut Vec<P>) {
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let d = sub(b, a);
        let (mut t0, mut t1) = (0f32, 1f32);
        let checks = [
            (-d.0, a.0 - x),
            (d.0, x + w - a.0),
            (-d.1, a.1 - y),
            (d.1, y + h - a.1),
        ];
        let mut inside = true;
        for (p, q) in checks {
            if p == 0.0 {
                if q < 0.0 {
                    inside = false;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
        if inside && t0 <= t1 {
            out.push(add(a, scale(d, t0)));
            out.push(add(a, scale(d, t1)));
        }
    }
}
