//! Continuous and raster exclusion shapes used by inline flow.

/// Форма обтекания плавающего блока — в координатах от НАЧАЛА стороны
/// обтекания (для правого флоата вызывающий зеркалит X заранее).
///
/// `cut(y0, y1)` — насколько занята строка-полоса [y0, y1): максимальный
/// горизонтальный экстент формы на этой полосе, в точках от края.
#[derive(Clone, Debug, PartialEq)]
pub enum FloatShape {
    /// Прямоугольник: занято `w` на высоте [top, top+h).
    Band { top: f32, h: f32, w: f32 },
    /// Круг с центром (cx, cy) от верха полосы флоата.
    Circle { top: f32, cx: f32, cy: f32, r: f32 },
    Ellipse {
        top: f32,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    /// Многоугольник: вершины в точках от начала стороны; экстент полосы —
    /// максимум X рёбер в её диапазоне.
    Poly {
        top: f32,
        pts: std::sync::Arc<Vec<(f32, f32)>>,
    },
    /// Continuous rounded-box geometry for each line band.
    RoundedBox {
        top: f32,
        off: f32,
        shape: std::sync::Arc<super::RoundedBox>,
    },
    /// Профиль из картинки (`shape-outside: url(...)`): экстент на каждую
    /// точку высоты, от начала стороны.
    Profile {
        top: f32,
        ext: std::sync::Arc<Vec<f32>>,
    },
}

impl FloatShape {
    /// Сдвинуть форму вниз: флоат перенесён на следующую полосу.
    pub fn shift_top(&mut self, dy: f32) {
        match self {
            FloatShape::Band { top, .. }
            | FloatShape::Circle { top, .. }
            | FloatShape::Ellipse { top, .. }
            | FloatShape::Poly { top, .. }
            | FloatShape::Profile { top, .. }
            | FloatShape::RoundedBox { top, .. } => *top += dy,
        }
    }

    pub fn cut(&self, y0: f32, y1: f32) -> f32 {
        match *self {
            FloatShape::Band { top, h, w } => {
                if top + h > y0 && top < y1 {
                    w
                } else {
                    0.0
                }
            }
            FloatShape::Circle { top, cx, cy, r } => ellipse_cut(top + cy, r, r, cx, y0, y1),
            FloatShape::Ellipse {
                top,
                cx,
                cy,
                rx,
                ry,
            } => ellipse_cut(top + cy, rx, ry, cx, y0, y1),
            FloatShape::RoundedBox {
                top,
                off,
                ref shape,
            } => {
                let cut = shape.cut(y0 - top, y1 - top);
                if cut > 0.0 { off + cut } else { 0.0 }
            }
            FloatShape::Profile { top, ref ext } => {
                let a = (y0 - top).max(0.0) as usize;
                let b = ((y1 - top).ceil()).max(0.0) as usize;
                return ext
                    .get(a..b.min(ext.len()))
                    .map(|s| s.iter().fold(0.0f32, |m, &v| m.max(v)))
                    .unwrap_or(0.0);
            }
            FloatShape::Poly { top, ref pts } => {
                // Верхняя кромка полосы ОТКРЫТА (как строки растра у
                // blink RasterShape): вершина/срез ровно на y0 принадлежит
                // предыдущей полосе — вогнутая «лесенка» иначе залипала
                // на ступени (shape-outside-polygon-007..011).
                let (y0, y1) = (y0 - top + 1e-3, y1 - top);
                let mut m = 0.0f32;
                let n = pts.len();
                for i in 0..n {
                    let (x1, py1) = pts[i];
                    let (x2, py2) = pts[(i + 1) % n];
                    // Вершина в полосе — как есть.
                    if py1 > y0 && py1 < y1 {
                        m = m.max(x1);
                    }
                    // Ребро пересекает границы полосы — X в точках среза.
                    let (lo, hi) = if py1 <= py2 { (py1, py2) } else { (py2, py1) };
                    if hi <= y0 || lo >= y1 || (hi - lo) < 1e-6 {
                        continue;
                    }
                    for yb in [y0.max(lo), y1.min(hi)] {
                        let t = (yb - py1) / (py2 - py1);
                        if (0.0..=1.0).contains(&t) {
                            m = m.max(x1 + (x2 - x1) * t);
                        }
                    }
                }
                m
            }
        }
    }

    /// Грубый ключ для кэша замера абзаца.
    pub fn hash_bits(&self) -> u64 {
        let q = |v: f32| (v * 8.0) as i64 as u64;
        match *self {
            FloatShape::Band { top, h, w } => {
                1 ^ q(top).rotate_left(8) ^ q(h).rotate_left(24) ^ q(w).rotate_left(40)
            }
            FloatShape::Circle { top, cx, cy, r } => {
                2 ^ q(top).rotate_left(6)
                    ^ q(cx).rotate_left(18)
                    ^ q(cy).rotate_left(30)
                    ^ q(r).rotate_left(44)
            }
            FloatShape::Ellipse {
                top,
                cx,
                cy,
                rx,
                ry,
            } => {
                3 ^ q(top).rotate_left(5)
                    ^ q(cx).rotate_left(15)
                    ^ q(cy).rotate_left(27)
                    ^ q(rx).rotate_left(39)
                    ^ q(ry).rotate_left(51)
            }
            FloatShape::RoundedBox {
                top,
                off,
                ref shape,
            } => 6 ^ q(top).rotate_left(6) ^ q(off).rotate_left(18) ^ shape.hash_bits(),
            FloatShape::Profile { top, ref ext } => ext
                .iter()
                .fold(5u64 ^ q(top), |acc, &v| acc.rotate_left(9) ^ q(v)),
            FloatShape::Poly { top, ref pts } => pts.iter().fold(4u64 ^ q(top), |acc, &(x, y)| {
                acc.rotate_left(7) ^ q(x) ^ q(y).rotate_left(3)
            }),
        }
    }
}

/// Экстент эллипса (центр по y — `cy_abs`, радиусы rx/ry, центр по x — cx)
/// на полосе [y0, y1): максимум `cx + rx·√(1−(dy/ry)²)` по dy в полосе.
pub(crate) fn ellipse_cut(cy_abs: f32, rx: f32, ry: f32, cx: f32, y0: f32, y1: f32) -> f32 {
    if ry <= 0.0 || rx <= 0.0 {
        return 0.0;
    }
    if y1 <= cy_abs - ry || y0 >= cy_abs + ry {
        return 0.0;
    }
    // Ближайшая к центру точка полосы даёт самый широкий срез.
    let dy = if (y0..y1).contains(&cy_abs) {
        0.0
    } else if y1 <= cy_abs {
        cy_abs - y1
    } else {
        y0 - cy_abs
    };
    let k = 1.0 - (dy / ry) * (dy / ry);
    if k <= 0.0 {
        return 0.0;
    }
    cx + rx * k.sqrt()
}
