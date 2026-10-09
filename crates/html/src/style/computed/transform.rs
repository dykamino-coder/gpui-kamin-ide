//! Transform: матрицы 2D/3D, перспектива, композиция.

/// `transform`: поворот, масштаб и сдвиг при отрисовке.
///
/// Сдвиг хранится вместе с поворотом: в CSS `translate()` внутри `transform`
/// и отдельное свойство `translate` складываются.
#[derive(Clone, Copy, Debug, PartialEq)]
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09): шаг 1 объёмных трансформаций
// (css-transforms-2) — полная накопленная 4x4 `m4`/`m4_pct`/`has_3d` рядом с
// плоской 2x3, `perspective`/`perspective-origin`/`transform-style`,
// `translateZ`/`scaleZ`/`rotateX|Y|3d`/`matrix3d` целиком, `preserve-3d`
// через потоко-локальный стек накопленных матриц и сплющивание плоскости
// z=0 на отрисовке (`interact::Transformed`), обёртка `transformed()` и без
// собственного `transform`. Срез 3029 пар (transforms/contain/overflow/
// masking/position/backgrounds): 2102 -> 2053, **+11/-60**; из потерь
// одиннадцать — 99.00 (`css-rotate-2d-3d-001`, `rotate3d-Z-*`,
// `css3-transform-rotateY`, `perspective-children-only-*`,
// `preserve3d-and-flattening-z-order-001/002`): страница разъезжается
// целиком, а не сдвигается. Возвращаться по одному рукаву: сначала
// `matrix3d`/`perspective()` внутри ОДНОГО элемента без стека, затем стек.
// План и патч — `target/scout-3d-2026-09.md` §7.
// Корень провала нашёл второй заход (`scout-3d-2026-09b.md`): хунк 7.18
// домножал на масштаб устройства весь столбец сдвига, включая m44
// (1 -> 1.25), а сплющивание делило на него всю матрицу — каждая коробка
// на объёмном пути сжималась в 0.8 вокруг transform-origin (0.75 % = ровно
// 125² − 100²). Узкий шаг 1' — 4x4 внутри ОДНОГО элемента, без стека и без
// обёртки элементов без `transform`, свёртка `S·M·S⁻¹` с нетронутым m44 —
// на том же срезе дал +14/-1 и внесён ниже.
pub struct Transform {
    pub rotate_rad: f32,
    /// Скос по осям в радианах (`skew`, `skewX`, `skewY`).
    pub skew_rad: (f32, f32),
    pub scale: (f32, f32),
    pub translate: (f32, f32),
    /// Сдвиг, заданный долями СОБСТВЕННОГО размера: `translate(-50%, -50%)`.
    /// Разрешается при отрисовке, когда размер известен.
    pub translate_pct: (f32, f32),
    /// Аффинная матрица всех функций В ПОРЯДКЕ ЗАПИСИ (css-transforms-1
    /// §transform-rendering: «multiply … from left to right»): линейная
    /// часть и сдвиг. Разложение выше складывает функции покомпонентно и
    /// порядок теряет (`translate(200px) rotate(180deg)` уводило коробку за
    /// экран) — рисует отрисовка по матрице, разложение остаётся для SVG и
    /// сдвига клипа.
    pub lin: [[f32; 2]; 2],
    /// Сдвиг по осям: пиксели, доля СОБСТВЕННОЙ ширины, доля высоты —
    /// проценты внутри цепочки складываются линейно и разрешаются при
    /// отрисовке.
    pub tr: [[f32; 3]; 2],
    /// Элемент m33 накопленной 4x4-матрицы. Плоская отрисовка его не видит,
    /// но `backface-visibility: hidden` прячет элемент ровно при m33 < 0
    /// (css-transforms-2 §backface-visibility). У плоских функций m33 = 1,
    /// поэтому множители перемножаются без потери точности.
    pub m33: f32,
    /// Полная 4×4 ОДНОГО элемента (css-transforms-2 §3d-transform-rendering),
    /// `m4[строка][столбец]`, столбец 3 — сдвиг в css-точках. Плоские функции
    /// вкладываются как есть, объёмные (`rotateX/Y/3d`, `translateZ`,
    /// `scaleZ`, `perspective()`, `matrix3d`) живут только здесь;
    /// `lin`/`tr` остаются для SVG, клипа и плоского пути отрисовки.
    pub m4: [[f32; 4]; 4],
    /// Доли СОБСТВЕННОГО размера в столбце сдвига: `m4_pct[строка] =
    /// [доля ширины, доля высоты]` (как `tr[i][1..3]`).
    pub m4_pct: [[f32; 2]; 4],
    /// Встретилась действительно объёмная функция: отрисовка идёт по `m4`,
    /// иначе — прежний плоский путь по `lin`/`tr`.
    pub has_3d: bool,
}

/// Ячейка матрицы перспективы элемента в точках устройства
/// (css-transforms-2 §perspective-matrix-computation): заводится при
/// разборе `perspective`, наполняется его `Transformed::paint`, читается
/// объёмным путём ПРЯМЫХ детей. Разделяемая ячейка, а не стек кадра:
/// абсолютный ребёнок с `z-index`/`fixed` рисуется отложенным слоем
/// (`defers`), когда `paint` родителя уже вышел; ячейка переживает кадр.
pub type PerspectiveFrame = std::rc::Rc<std::cell::Cell<Option<[[f32; 4]; 4]>>>;

/// Ячейка объёмного контекста `transform-style: preserve-3d`
/// (css-transforms-2 §accumulated-3d-transformation-matrix): накопленная
/// 4×4 в точках устройства И собственная аффинная доля
/// `[[a, b, tx], [c, d, ty]]`, которую владелец уже втолкнул в gpui.
/// Ребёнок кладёт себя по `flatten(A · C)`, а родительскую долю обязан
/// снять сам: `with_transformation` складывает вложения как `inner∘outer`
/// (`vendor/gpui/src/window.rs:2789`; обратный порядок ЗАМЕРЕН И ОТКАЧЕН —
/// css-writing-modes −9). Ячейка, а не стек кадра, — по той же причине,
/// что у перспективы: абсолютный ребёнок с `z-index`/`fixed` рисуется
/// отложенным слоем, когда `paint` владельца уже вышел.
pub type Frame3d = std::rc::Rc<std::cell::Cell<Option<([[f32; 4]; 4], [[f32; 3]; 2])>>>;

/// Единичная 4×4.
pub const IDENTITY4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Произведение 4×4: `a · b`.
pub fn mul4(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// Определитель 4×4 (разложение по первой строке через миноры 3×3).
pub fn det4(m: &[[f32; 4]; 4]) -> f32 {
    let minor = |r: [usize; 3], c: [usize; 3]| -> f32 {
        let a = |i: usize, j: usize| m[r[i]][c[j]];
        a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1))
            - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
            + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0))
    };
    m[0][0] * minor([1, 2, 3], [1, 2, 3]) - m[0][1] * minor([1, 2, 3], [0, 2, 3])
        + m[0][2] * minor([1, 2, 3], [0, 1, 3])
        - m[0][3] * minor([1, 2, 3], [0, 1, 2])
}

/// Гомография плоскости z=0 → экран: строки/столбцы 0,1,3 полной матрицы.
/// Её вырождение — плоскость видна ребром (`rotateX(90deg)`), даже когда
/// сама 4×4 обратима.
pub fn det3_plane(m: &[[f32; 4]; 4]) -> f32 {
    let idx = [0usize, 1, 3];
    let a = |i: usize, j: usize| m[idx[i]][idx[j]];
    a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1))
        - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0))
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            rotate_rad: 0.0,
            skew_rad: (0.0, 0.0),
            scale: (1.0, 1.0),
            translate: (0.0, 0.0),
            translate_pct: (0.0, 0.0),
            lin: [[1.0, 0.0], [0.0, 1.0]],
            tr: [[0.0; 3]; 2],
            m33: 1.0,
            m4: IDENTITY4,
            m4_pct: [[0.0; 2]; 4],
            has_3d: false,
        }
    }
}

impl Transform {
    /// Поворот вокруг произвольной оси, СПЛЮЩЕННЫЙ на плоскость экрана.
    ///
    /// Сплющивание (css-transforms-2 §3d-transform-rendering) — это
    /// вычёркивание третьей строки и третьего столбца 4x4-матрицы, поэтому
    /// плоская часть — ровно верхний 2x2 блок матрицы поворота Родрига, а
    /// m33 = z²(1-cos a) + cos a нужен для `backface-visibility`.
    /// Проверка: ось Z даёт обычный поворот, ось Y — diag(cos a, 1), ось X —
    /// diag(1, cos a), то есть ровно `rotateZ`/`rotateY`/`rotateX`.
    pub fn axis_rot(x: f32, y: f32, z: f32, a: f32) -> ([[f32; 2]; 2], f32) {
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 {
            return ([[1.0, 0.0], [0.0, 1.0]], 1.0);
        }
        let (x, y, z) = (x / len, y / len, z / len);
        let (c, s) = (a.cos(), a.sin());
        let k = 1.0 - c;
        (
            [
                [x * x * k + c, x * y * k - z * s],
                [y * x * k + z * s, y * y * k + c],
            ],
            z * z * k + c,
        )
    }
    /// Дописать плоскую функцию справа в ОБЕ матрицы: `M := M · [l | v]`.
    pub(crate) fn push(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
        self.push2(l, v);
        let mut f = IDENTITY4;
        f[0][0] = l[0][0];
        f[0][1] = l[0][1];
        f[1][0] = l[1][0];
        f[1][1] = l[1][1];
        f[0][3] = v[0][0];
        f[1][3] = v[1][0];
        let pct = [
            [v[0][1], v[0][2]],
            [v[1][1], v[1][2]],
            [0.0, 0.0],
            [0.0, 0.0],
        ];
        self.push4(f, pct);
    }

    /// Только плоская 2×3 (`lin`/`tr`) — для объёмных функций, чья
    /// сплющенная тень нужна SVG и клипу.
    pub(crate) fn push2(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
        let m = self.lin;
        self.lin = [
            [
                m[0][0] * l[0][0] + m[0][1] * l[1][0],
                m[0][0] * l[0][1] + m[0][1] * l[1][1],
            ],
            [
                m[1][0] * l[0][0] + m[1][1] * l[1][0],
                m[1][0] * l[0][1] + m[1][1] * l[1][1],
            ],
        ];
        for i in 0..2 {
            for k in 0..3 {
                self.tr[i][k] += m[i][0] * v[0][k] + m[i][1] * v[1][k];
            }
        }
    }

    /// Дописать 4×4 справа: `M4 := M4 · f`. Столбец сдвига несёт доли размера:
    /// `(M·F)[i][3] = Σ_k M[i][k]·F[k][3]`, где `F[k][3]` для k<3 — «точки +
    /// доля», а `F[3][3]` домножает уже накопленные доли самой `M`.
    pub(crate) fn push4(&mut self, f: [[f32; 4]; 4], pct: [[f32; 2]; 4]) {
        let m = self.m4;
        let mut p = [[0.0f32; 2]; 4];
        for i in 0..4 {
            for a in 0..2 {
                p[i][a] =
                    self.m4_pct[i][a] * f[3][3] + (0..3).map(|k| m[i][k] * pct[k][a]).sum::<f32>();
            }
        }
        self.m4 = mul4(m, f);
        self.m4_pct = p;
    }

    /// `translate3d(x, y, z)` в точках.
    pub fn translate4(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[0][3] = x;
        m[1][3] = y;
        m[2][3] = z;
        m
    }

    /// `scale3d(x, y, z)`.
    pub fn scale4(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[0][0] = x;
        m[1][1] = y;
        m[2][2] = z;
        m
    }

    /// `perspective(d)`: m34 = −1/d (css-transforms-2 §perspective()); d уже
    /// не меньше 1px — clamp делает вызывающий.
    pub fn perspective4(d: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[3][2] = -1.0 / d;
        m
    }

    /// `rotate3d(x, y, z, a)`: R = cos·I + sin·[u]× + (1−cos)·u·uᵀ — верхний
    /// 2×2 блок и m33 ровно те же, что в `axis_rot`.
    pub fn rot4(x: f32, y: f32, z: f32, a: f32) -> [[f32; 4]; 4] {
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 {
            return IDENTITY4;
        }
        let (x, y, z) = (x / len, y / len, z / len);
        let (c, s) = (a.cos(), a.sin());
        let k = 1.0 - c;
        [
            [x * x * k + c, x * y * k - z * s, x * z * k + y * s, 0.0],
            [y * x * k + z * s, y * y * k + c, y * z * k - x * s, 0.0],
            [z * x * k - y * s, z * y * k + x * s, z * z * k + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

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

    pub(crate) fn rot(a: f32) -> [[f32; 2]; 2] {
        [[a.cos(), -a.sin()], [a.sin(), a.cos()]]
    }

    pub(crate) fn diag(x: f32, y: f32) -> [[f32; 2]; 2] {
        [[x, 0.0], [0.0, y]]
    }
}

pub(super) const NO_SHIFT: [[f32; 3]; 2] = [[0.0; 3]; 2];
