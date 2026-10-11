//! Методы Transform: чистый сдвиг, композиция (then), индивидуальные rotate/scale, интерполяция 2D.

use super::*;

impl Transform {
    /// Список — чистый плоский сдвиг в css-точках (`translate*()`/`matrix`
    /// с единичной линейной частью, без долей размера и без объёма):
    /// `Some((x, y))`.
    pub fn pure_px_shift(&self) -> Option<(f32, f32)> {
        let id2 = self.lin == [[1.0, 0.0], [0.0, 1.0]];
        let no_pct = self.tr[0][1] == 0.0
            && self.tr[0][2] == 0.0
            && self.tr[1][1] == 0.0
            && self.tr[1][2] == 0.0
            && self.m4_pct.iter().all(|r| r[0] == 0.0 && r[1] == 0.0);
        let mut m = self.m4;
        m[0][3] = 0.0;
        m[1][3] = 0.0;
        let (x, y) = (self.tr[0][0], self.tr[1][0]);
        (id2 && no_pct
            && !self.has_3d
            && m == IDENTITY4
            && self.m4[0][3] == x
            && self.m4[1][3] == y
            && x.is_finite()
            && y.is_finite())
        .then_some((x, y))
    }

    /// Домножить СПРАВА на уже накопленную матрицу другого объявления.
    ///
    /// Нужно слоению motion-1: offset-трансформ идёт ПЕРЕД авторским
    /// `transform`, а разбор авторского уже сложил свою матрицу — её
    /// приходится приставлять целиком, а не по одной функции.
    pub fn then(mut self, other: &Transform) -> Transform {
        self.push(other.lin, other.tr);
        self.rotate_rad += other.rotate_rad;
        self.skew_rad.0 += other.skew_rad.0;
        self.skew_rad.1 += other.skew_rad.1;
        self.scale.0 *= other.scale.0;
        self.scale.1 *= other.scale.1;
        self.translate.0 += other.translate.0;
        self.translate.1 += other.translate.1;
        self.translate_pct.0 += other.translate_pct.0;
        self.translate_pct.1 += other.translate_pct.1;
        self.m33 *= other.m33;
        self
    }

    /// Отдельные `rotate`/`scale` СЛЕВА от этого списка (css-transforms-2
    /// §ctm: п.4 — `rotate`, п.5 — `scale`, п.7 — функции `transform`; Blink
    /// `ComputedStyle::ApplyTransform`, style/computed_style.cc:1464-1487 —
    /// `Rotate()`, `Scale()`, затем `Transform().Operations()`). Обе матрицы —
    /// плоская и 4×4 — получают одну и ту же свёртку; разложение, m33 и
    /// `has_3d` остаются от самого списка (SVG, клип, изнанка).
    pub fn after_individual(self, rotate: Option<f32>, scale: Option<(f32, f32)>) -> Transform {
        if rotate.is_none() && scale.is_none() {
            return self;
        }
        let mut out = Transform::default();
        if let Some(a) = rotate {
            out.push(Self::rot(a), NO_SHIFT);
        }
        if let Some((x, y)) = scale {
            out.push(Self::diag(x, y), NO_SHIFT);
        }
        out.push2(self.lin, self.tr);
        out.push4(self.m4, self.m4_pct);
        Transform {
            lin: out.lin,
            tr: out.tr,
            m4: out.m4,
            m4_pct: out.m4_pct,
            ..self
        }
    }

    /// Промежуточная ПЛОСКАЯ матрица между `self` и `other` на доле `k`
    /// (css-transforms-1 §matrix-interpolation): обе раскладываются на
    /// масштаб, угол и остаток 2×2 (§decomposing-a-2d-matrix, «unmatrix»),
    /// компоненты смешиваются линейно — со сменой флипа и без «длинного пути»
    /// (§interpolation-of-decomposed-2d-matrix-values) — и собираются обратно:
    /// `lin = K·R(угол)·diag(sx, sy)`. `none` приходит сюда тождеством
    /// (§interpolation-of-transforms). Сдвиг вместе с долями размера —
    /// линейно. Необратимая сторона — `None`: анимация дискретна.
    pub fn lerp_2d(&self, other: &Transform, k: f32) -> Option<Transform> {
        use std::f32::consts::{PI, TAU};
        // Столбцы `lin` — образы осей: `row0` псевдокода = (a, b) записи
        // `matrix(a, b, c, d, e, f)`, `row1` = (c, d).
        let unmatrix = |l: [[f32; 2]; 2]| -> Option<((f32, f32), f32, [f32; 4])> {
            let (r0x, r0y, r1x, r1y) = (l[0][0], l[1][0], l[0][1], l[1][1]);
            let det = r0x * r1y - r0y * r1x;
            if det.abs() < 1e-9 {
                return None;
            }
            let mut sx = (r0x * r0x + r0y * r0y).sqrt();
            let mut sy = (r1x * r1x + r1y * r1y).sqrt();
            if det < 0.0 {
                if r0x < r1y {
                    sx = -sx;
                } else {
                    sy = -sy;
                }
            }
            let (r0x, r0y, r1x, r1y) = (r0x / sx, r0y / sx, r1x / sy, r1y / sy);
            let angle = r0y.atan2(r0x);
            let (sn, cs) = (-r0y, r0x);
            let m = [
                cs * r0x + sn * r1x,
                cs * r0y + sn * r1y,
                -sn * r0x + cs * r1x,
                -sn * r0y + cs * r1y,
            ];
            Some(((sx, sy), angle, m))
        };
        let (mut sa, mut aa, ma) = unmatrix(self.lin)?;
        let (sb, mut ab, mb) = unmatrix(other.lin)?;
        if (sa.0 < 0.0 && sb.1 < 0.0) || (sa.1 < 0.0 && sb.0 < 0.0) {
            sa = (-sa.0, -sa.1);
            aa += if aa < 0.0 { PI } else { -PI };
        }
        if aa == 0.0 {
            aa = TAU;
        }
        if ab == 0.0 {
            ab = TAU;
        }
        if (aa - ab).abs() > PI {
            if aa > ab {
                aa -= TAU;
            } else {
                ab -= TAU;
            }
        }
        let mix = |x: f32, y: f32| x + (y - x) * k;
        let (sx, sy) = (mix(sa.0, sb.0), mix(sa.1, sb.1));
        let m: [f32; 4] = std::array::from_fn(|i| mix(ma[i], mb[i]));
        let angle = mix(aa, ab);
        let r = Self::rot(angle);
        // K·R — остаток столбцами (m11, m12) и (m21, m22), как в псевдокоде.
        let kr = [
            [
                m[0] * r[0][0] + m[2] * r[1][0],
                m[0] * r[0][1] + m[2] * r[1][1],
            ],
            [
                m[1] * r[0][0] + m[3] * r[1][0],
                m[1] * r[0][1] + m[3] * r[1][1],
            ],
        ];
        let lin = [
            [kr[0][0] * sx, kr[0][1] * sy],
            [kr[1][0] * sx, kr[1][1] * sy],
        ];
        let tr: [[f32; 3]; 2] =
            std::array::from_fn(|i| std::array::from_fn(|j| mix(self.tr[i][j], other.tr[i][j])));
        let mut out = Transform::default();
        out.push(lin, tr);
        Some(Transform {
            rotate_rad: angle,
            scale: (sx, sy),
            translate: (tr[0][0], tr[1][0]),
            translate_pct: (tr[0][1], tr[1][2]),
            ..out
        })
    }
}
