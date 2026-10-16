//! Один угол контура (css-borders-4 §corner-shape-rendering) в модели Blink
//! `ContouredRect::Corner`: четыре вершины по часовой (начало, внешняя
//! вершина, конец, центр) и показатель суперэллипса `2^K`. Отсюда же —
//! кривая угла: прямые для bevel/notch/square, коника для round и пара
//! кубических Безье для прочих K (`ApproximateSuperellipseHalfCornerAs-
//! BezierCurve`, platform/geometry/path_builder.cc). Кривую спека оставляет
//! «implementation-approximated»; эталоны WPT нарисованы этими же Безье.

use super::{P, Seg, add, scale, sub};

/// Показатель у прямого угла: K ≥ 16 (`CornerCurvature::kStraight`).
pub(crate) const STRAIGHT: f32 = 1000.0;
/// Показатель полной выемки (`kNotch = 1 / kStraight`).
pub(crate) const NOTCH: f32 = 1.0 / STRAIGHT;
/// Показатель скошенного угла (`kBevel`) и круглого (`kRound`).
pub(crate) const BEVEL: f32 = 1.0;
pub(crate) const ROUND: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Corner {
    pub(crate) v: [P; 4],
    pub(crate) curvature: f32,
}

impl Corner {
    /// Угол из прямоугольника угла `(x, y, w, h)`: вершины [лево-верх,
    /// право-верх, право-низ, лево-низ], сдвинутые на `rotation` (0 — правый
    /// верхний угол коробки, 1 — правый нижний, …). Показатель НЕ зажимается
    /// (как конструктор Blink от прямоугольника).
    pub(crate) fn of_rect(
        (x, y, w, h): (f32, f32, f32, f32),
        rotation: usize,
        curvature: f32,
    ) -> Self {
        let mut v = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        v.rotate_left(rotation % 4);
        Corner { v, curvature }
    }

    /// Угол из вершин; показатель зажат в [kNotch, kStraight].
    pub(crate) fn of_vertices(v: [P; 4], curvature: f32) -> Self {
        Corner {
            v,
            curvature: curvature.clamp(NOTCH, STRAIGHT),
        }
    }

    pub(crate) fn start(&self) -> P {
        self.v[0]
    }
    pub(crate) fn outer(&self) -> P {
        self.v[1]
    }
    pub(crate) fn end(&self) -> P {
        self.v[2]
    }
    pub(crate) fn center(&self) -> P {
        self.v[3]
    }
    pub(crate) fn v1(&self) -> P {
        sub(self.outer(), self.start())
    }
    pub(crate) fn v2(&self) -> P {
        sub(self.end(), self.outer())
    }
    pub(crate) fn v3(&self) -> P {
        sub(self.center(), self.end())
    }
    pub(crate) fn v4(&self) -> P {
        sub(self.start(), self.center())
    }
    /// Прямой угол. Допуск: обращение выемки `1 / kNotch` в f32 даёт
    /// 999.99994, а не ровно 1000 (кубика Blink там неотличима от прямой).
    pub(crate) fn is_straight(&self) -> bool {
        self.curvature >= STRAIGHT * 0.999
    }
    pub(crate) fn is_notch(&self) -> bool {
        self.curvature <= NOTCH / 0.999
    }
    pub(crate) fn is_empty(&self) -> bool {
        len(self.v1()) == 0.0 || len(self.v2()) == 0.0
    }

    /// Вогнутый угол рисуется выпуклым с обменом центра и внешней вершины.
    fn inverse(&self) -> Self {
        Corner::of_vertices(
            [self.start(), self.center(), self.end(), self.outer()],
            1.0 / self.curvature,
        )
    }

    /// Точка единичного угла: (x — к внешней стороне, y — к началу).
    fn map(&self, (x, y): P) -> P {
        add(self.center(), add(scale(self.v1(), x), scale(self.v4(), y)))
    }
}

pub(crate) fn len((x, y): P) -> f32 {
    x.hypot(y)
}

/// Середина кривой при t = 0.5 (`HalfCornerForCurvature`).
pub(crate) fn half_corner(curvature: f32) -> f32 {
    0.5f64.powf(1.0 / curvature.clamp(NOTCH, STRAIGHT) as f64) as f32
}

/// Два контрольных и конечная точка Безье для половины выпуклого угла (от
/// t = 0 до 45°) в единичной системе угла — подгонка Blink.
fn half_corner_bezier(curvature: f32) -> [P; 3] {
    const C: [f64; 7] = [
        1.2430920942724248,
        2.010479023614843,
        0.32922901179443753,
        0.2823023142212073,
        1.3473704261055421,
        2.9149468637949814,
        0.9106507102917086,
    ];
    let s = (curvature as f64).log2() as f32 as f64;
    let slope = (C[0] + (C[6] - C[0]) * 0.5 * (1.0 + (C[5] * (s - C[1])).tanh())) as f32 as f64;
    let base = (1.0 / (1.0 + (-slope * (0.0 - C[1])).exp())) as f32;
    let logistic = (1.0 / (1.0 + (-slope * (s - C[1])).exp())) as f32;
    let a = (logistic - base) / (1.0 - base);
    let b = (C[2] * (-C[3] * s.powf(C[4])).exp()) as f32;
    let h = half_corner(curvature);
    [(a, 1.0), (h - b, h + b), (h, h)]
}

/// Дописывает угол к контуру (`AddCurvedCorner`): линия к началу угла и
/// сама кривая до его конца.
pub(crate) fn add_curved_corner(path: &mut Vec<Seg>, corner: &Corner) {
    if corner.curvature < 1.0 {
        add_curved_corner(path, &corner.inverse());
        return;
    }
    path.push(Seg::Line(corner.start()));
    if corner.is_straight() || corner.is_empty() {
        path.push(Seg::Line(corner.outer()));
        path.push(Seg::Line(corner.end()));
    } else if corner.curvature == BEVEL {
        path.push(Seg::Line(corner.end()));
    } else if corner.curvature == ROUND {
        // Коника с весом √2/2 — четверть эллипса в аффинной рамке угла;
        // кубика с κ = 4w/(3(1+w)) — её стандартная запись.
        let k =
            4.0 * std::f32::consts::FRAC_1_SQRT_2 / (3.0 * (1.0 + std::f32::consts::FRAC_1_SQRT_2));
        let (s, o, e) = (corner.start(), corner.outer(), corner.end());
        path.push(Seg::Cubic(
            add(s, scale(sub(o, s), k)),
            add(e, scale(sub(o, e), k)),
            e,
        ));
    } else {
        let [c0, c1, c2] = half_corner_bezier(corner.curvature);
        let t = |(x, y): P| (y, x);
        path.push(Seg::Cubic(corner.map(c0), corner.map(c1), corner.map(c2)));
        path.push(Seg::Cubic(
            corner.map(t(c1)),
            corner.map(t(c0)),
            corner.end(),
        ));
    }
}
