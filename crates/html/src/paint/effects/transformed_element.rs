//! Элемент `Transformed`.
// owner: A

mod lifecycle;

use gpui::{AnyElement, IntoElement, Pixels};

pub struct Transformed {
    pub(crate) child: Option<AnyElement>,
    /// Поворот в радианах, масштаб по осям, сдвиг в точках.
    pub rotate: f32,
    /// Скос по осям в радианах (`transform: skew`).
    pub skew: (f32, f32),
    pub scale: (f32, f32),
    pub translate: (f32, f32),
    /// Сдвиг долями СОБСТВЕННОГО размера: `translate(-50%, -50%)`.
    pub translate_pct: (f32, f32),
    /// Доли от размера элемента: 0.5, 0.5 — центр.
    pub origin: (f32, f32),
    /// Точка отсчёта В ТОЧКАХ по осям — сильнее доли, когда задана.
    pub origin_px: (Option<f32>, Option<f32>),
    /// Матрица функций в порядке записи (см. `computed::Transform::lin`).
    pub lin: [[f32; 2]; 2],
    /// Сдвиг: пиксели, доля ширины, доля высоты.
    pub tr: [[f32; 3]; 2],
    /// Полная 4×4 элемента и доли размера в её столбце сдвига
    /// (`computed::Transform::m4`/`m4_pct`); `has_3d` — идти по ней.
    pub m4: [[f32; 4]; 4],
    pub m4_pct: [[f32; 2]; 4],
    pub has_3d: bool,
    /// `backface-visibility: hidden` — решается по `m4[2][2]` на отрисовке,
    /// коробка держит место.
    pub backface_hidden: bool,
    /// Третья координата `transform-origin` в css-точках.
    pub origin_z: Option<f32>,
    /// Своя `perspective` (css-точки, ≥ 1px), её точка отсчёта долями и
    /// точками по осям, и ячейка, куда `paint` кладёт T(po)·P(d)·T(−po) в
    /// точках устройства — для объёмных детей.
    pub perspective: Option<f32>,
    pub perspective_origin: (f32, f32),
    pub perspective_origin_px: (Option<f32>, Option<f32>),
    pub perspective_frame: Option<crate::style::computed::PerspectiveFrame>,
    /// Ячейка ПРЯМОГО родителя: объёмный путь домножает на неё слева
    /// (css-transforms-2 §3d-transform-rendering, п.3).
    pub under_perspective: Option<crate::style::computed::PerspectiveFrame>,
    /// Своя ячейка объёмного контекста (`transform-style: preserve-3d`):
    /// `paint` кладёт в неё накопленную 4×4 и свою аффинную долю ДО детей.
    pub frame_3d: Option<crate::style::computed::Frame3d>,
    /// Ячейка объёмного контекста ПРЯМОГО родителя: своя матрица копится
    /// поверх неё, изнанка решается по накопленной, доля родителя снимается.
    pub under_3d: Option<crate::style::computed::Frame3d>,
    /// Чистый плоский сдвиг уже перенесён в место раскладки на подготовке
    /// (`prepaint`, `Window::set_layout_placed_origin`): `paint` рисует без
    /// матрицы.
    pub(crate) placed: bool,
    /// Неокруглённое место коробки из подготовки: осевой поворот на
    /// отрисовке округляет края от него (`layout_origin_unrounded` доступен
    /// только до отрисовки).
    pub(crate) exact_origin: Option<gpui::Point<Pixels>>,
    /// Reference box shared by the cells of a transformed table row or row
    /// group (css-transforms-1 §transformable-element: the row has no box
    /// of its own in our grid): every cell unions its unrounded box into it
    /// on prepaint, and paint resolves origin/percentages against it.
    pub ref_box: Option<RefBox>,
}

/// Union of unrounded boxes, filled on prepaint (see `Transformed::ref_box`).
pub type RefBox = std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>>;

/// Сплющивание плоскости z=0 в аффинную матрицу экрана
/// (css-transforms-2 §3d-transform-rendering).
///
/// Точка плоскости (x, y, 0, 1) уходит в (F0·p, F1·p, ·, F3·p); экран —
/// деление на w = F3·p. Если w не зависит от x и y (строка 3 без x/y —
/// весь класс `translateZ` + `perspective()`), результат ТОЧНО аффинный.
/// Иначе (rotateX/Y под `perspective()` — трапеция, которую квад gpui не
/// рисует) берём касательную аффинную карту в центре коробки:
/// детерминированно и одинаково для теста и эталона с той же гомографией
/// (transform3d-matrix3d-003/-004). `None` — плоскость за глазом или ребром.
fn flatten_plane(f: &[[f32; 4]; 4], center: (f32, f32)) -> Option<gpui::TransformationMatrix> {
    const EPS: f32 = 1e-5;
    if crate::style::computed::det3_plane(f).abs() < EPS {
        return None;
    }
    let (cx, cy) = center;
    if f[3][0].abs() < 1e-9 && f[3][1].abs() < 1e-9 {
        let w = f[3][3];
        if w <= 1e-9 {
            return None;
        }
        return Some(gpui::TransformationMatrix {
            rotation_scale: [[f[0][0] / w, f[0][1] / w], [f[1][0] / w, f[1][1] / w]],
            translation: [f[0][3] / w, f[1][3] / w],
        });
    }
    let wc = f[3][0] * cx + f[3][1] * cy + f[3][3];
    if wc <= 1e-6 {
        return None;
    }
    let x = (f[0][0] * cx + f[0][1] * cy + f[0][3]) / wc;
    let y = (f[1][0] * cx + f[1][1] * cy + f[1][3]) / wc;
    let j = |i: usize, k: usize, v: f32| (f[i][k] - v * f[3][k]) / wc;
    let rs = [[j(0, 0, x), j(0, 1, x)], [j(1, 0, y), j(1, 1, y)]];
    Some(gpui::TransformationMatrix {
        rotation_scale: rs,
        translation: [
            x - rs[0][0] * cx - rs[0][1] * cy,
            y - rs[1][0] * cx - rs[1][1] * cy,
        ],
    })
}

/// Обратная аффинная `[[a, b, tx], [c, d, ty]]`.
///
/// Нужна ровно затем, чтобы снять долю родителя: gpui складывает вложенные
/// `with_transformation` как `inner∘outer` (`window.rs:2789`; обратный
/// порядок замерен и откачен), поэтому ребёнок объёмного контекста, желая
/// оказаться на абсолютной `G`, обязан втолкнуть `G ∘ F_родителя⁻¹`.
fn invert_affine(m: [[f32; 3]; 2]) -> Option<gpui::TransformationMatrix> {
    let (a, b, tx) = (m[0][0], m[0][1], m[0][2]);
    let (c, d, ty) = (m[1][0], m[1][1], m[1][2]);
    let det = a * d - b * c;
    if det.abs() < 1e-9 {
        return None;
    }
    Some(gpui::TransformationMatrix {
        rotation_scale: [[d / det, -b / det], [-c / det, a / det]],
        translation: [(b * ty - d * tx) / det, (c * tx - a * ty) / det],
    })
}

impl Transformed {
    pub fn new(child: AnyElement) -> Self {
        Transformed {
            child: Some(child),
            rotate: 0.0,
            skew: (0.0, 0.0),
            scale: (1.0, 1.0),
            translate: (0.0, 0.0),
            translate_pct: (0.0, 0.0),
            origin: (0.5, 0.5),
            origin_px: (None, None),
            lin: [[1.0, 0.0], [0.0, 1.0]],
            tr: [[0.0; 3]; 2],
            m4: crate::style::computed::IDENTITY4,
            m4_pct: [[0.0; 2]; 4],
            has_3d: false,
            backface_hidden: false,
            origin_z: None,
            perspective: None,
            perspective_origin: (0.5, 0.5),
            perspective_origin_px: (None, None),
            perspective_frame: None,
            under_perspective: None,
            frame_3d: None,
            under_3d: None,
            placed: false,
            exact_origin: None,
            ref_box: None,
        }
    }

    /// Плоская матрица — чистый сдвиг (линейная часть единичная с точностью
    /// до ошибки `f32`: `rotate(360deg)` даёт sin ≈ 1e-7): сдвиг в css-точках
    /// для коробки `w × h`.
    pub(crate) fn pure_shift(&self, w: f32, h: f32) -> Option<(f32, f32)> {
        let [[a, b], [c, d]] = self.lin;
        let eps = 1e-5;
        let id = (a - 1.0).abs() < eps && b.abs() < eps && c.abs() < eps && (d - 1.0).abs() < eps;
        let sx = self.tr[0][0] + w * self.tr[0][1] + h * self.tr[0][2];
        let sy = self.tr[1][0] + w * self.tr[1][1] + h * self.tr[1][2];
        (id && sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }
}

impl IntoElement for Transformed {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
