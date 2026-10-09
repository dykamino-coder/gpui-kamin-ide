//! Элемент `Transformed`.
// owner: A

use crate::paint::effects::transform_geometry::quarter_turn;
use gpui::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window, px};

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
        translation: [x - rs[0][0] * cx - rs[0][1] * cy, y - rs[1][0] * cx - rs[1][1] * cy],
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

impl Element for Transformed {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Плоская матрица — на стек якорей (`anchor::tf_push`): рамку якоря
        // проба снимает на подготовке, а трансформ применяется только в
        // `paint`, и без стека якорь виделся до трансформа (css-anchor-
        // position-1 §2 «includes … transforms»; `transform-001/002/009`).
        // Формула та же, что у плоского пути `paint`, но в css-точках, без
        // `scale_factor`: x' = o + lin·(x − o) + сдвиг. Объёмный путь в стек
        // не идёт — его матрица решается на отрисовке по накопленной ячейке.
        let flat = !self.has_3d && self.frame_3d.is_none() && self.under_3d.is_none();
        // Чистый плоский сдвиг — смена начала координат (css-transforms-1
        // §transform-rendering): коробка обязана рисоваться байт в байт как
        // разложенная на сдвинутом месте. Матрицей дробный сдвиг устройства
        // (10px × 1.25) ложился ПОСЛЕ округления краёв раскладки и выбора
        // подпикселя глифов — края и текст расходились на точку с эталоном
        // на `top/left` (Blink так же проносит дробное смещение сквозь
        // 2D-сдвиг: `PaintPropertyTreeBuilder`, subpixel accumulation).
        // Поддерево переносится до округления (`set_layout_placed_origin`,
        // тот же механизм у `LatePlace`), края округляются на конечном месте.
        self.placed = false;
        self.exact_origin = Some(window.layout_origin_unrounded(*layout_id));
        if let Some(r) = self.ref_box.as_ref() {
            let own = gpui::Bounds {
                origin: window.layout_origin_unrounded(*layout_id),
                size: window.layout_size_unrounded(*layout_id),
            };
            r.set(Some(r.get().map_or(own, |u| u.union(&own))));
        }
        if flat && self.perspective.is_none() {
            let size = window.layout_size_unrounded(*layout_id);
            if let Some((sx, sy)) =
                self.pure_shift(f32::from(size.width), f32::from(size.height))
            {
                let origin = window.layout_origin_unrounded(*layout_id);
                window.set_layout_placed_origin(*layout_id, origin + gpui::point(px(sx), px(sy)));
                self.placed = true;
                self.child
                    .as_mut()
                    .unwrap()
                    .prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
                return;
            }
        }
        if flat {
            let (w, h) = {
                let exact = window.layout_size_unrounded(*layout_id);
                (f32::from(exact.width), f32::from(exact.height))
            };
            let origin = self.scaled_origin(bounds.origin);
            let ox = f32::from(origin.x) + w * self.origin.0 + self.origin_px.0.unwrap_or(0.0);
            let oy = f32::from(origin.y) + h * self.origin.1 + self.origin_px.1.unwrap_or(0.0);
            let sx = self.tr[0][0] + w * self.tr[0][1] + h * self.tr[0][2];
            let sy = self.tr[1][0] + w * self.tr[1][1] + h * self.tr[1][2];
            let [[a, b], [c, d]] = self.lin;
            crate::anchor::tf_push([
                [a, b, ox - a * ox - b * oy + sx],
                [c, d, oy - c * ox - d * oy + sy],
            ]);
        }
        self.child.as_mut().unwrap().prepaint(window, cx);
        if flat {
            crate::anchor::tf_pop();
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.placed {
            self.child.as_mut().unwrap().paint(window, cx);
            return;
        }
        let scale_factor = window.scale_factor();
        // CSS transform origins precede device-pixel snapping (Transforms 1 §3).
        let raw_origin = self.exact_origin.unwrap_or(bounds.origin);
        let scaled_origin = self.scaled_origin(bounds.origin);
        let dev = |v: f32| px(v).scale(scale_factor);
        // Точка отсчёта — в устройстве, от неё и разворачиваем. Записанная
        // длиной, она сильнее доли: `transform-origin: 0 0` — левый верх, а
        // не центр (доля из длины считается только здесь, где размер известен).
        // Доля × размер ПЛЮС точки: `calc(50% + 10px)` — смесь, и доля у
        // чистых точек равна нулю (css-transforms-1 §5.2).
        // Доли (`transform-origin: 50%`, `translate(100%)`) — от размера
        // раскладки, а не от округлённых к точке устройства краёв: 50px ×
        // 1.25 = 62.5 округлялось до 63, и `translateY(100%)` уезжал на
        // 0.4px от `translateY(50px)` (`transform-percent-*`).
        let exact = window.layout_size_unrounded(*layout_id);
        let (w, h) = (f32::from(exact.width), f32::from(exact.height));
        // Reference box of a table row/row group spread over its cells
        // (`ref_box`): origin and percentages resolve against it, offset
        // from this cell's own box.
        let (rw, rh, rdx, rdy) = match self.ref_box.as_ref().and_then(|r| r.get()) {
            Some(r) => (
                f32::from(r.size.width),
                f32::from(r.size.height),
                f32::from(r.origin.x - raw_origin.x),
                f32::from(r.origin.y - raw_origin.y),
            ),
            None => (w, h, 0.0, 0.0),
        };
        let ox = rdx + rw * self.origin.0 + self.origin_px.0.unwrap_or(0.0);
        let oy = rdy + rh * self.origin.1 + self.origin_px.1.unwrap_or(0.0);
        let origin = gpui::point(
            dev(f32::from(scaled_origin.x) + ox),
            dev(f32::from(scaled_origin.y) + oy),
        );
        let back = gpui::point(
            dev(-(f32::from(scaled_origin.x) + ox)),
            dev(-(f32::from(scaled_origin.y) + oy)),
        );
        // Матрица функций в порядке записи (css-transforms-1
        // §transform-rendering), вокруг точки отсчёта: она уводится в ноль и
        // возвращается. Проценты сдвига считаются от собственного размера —
        // он известен только здесь, на отрисовке.
        let shift = |row: [f32; 3]| (row[0] + rw * row[1] + rh * row[2]) * scale_factor;
        // Изнанка (css-transforms-2 §backface-visibility): элемент разложен и
        // держит место, но не рисуется. m33 — из полной 4×4 самого элемента;
        // у плоских функций он равен 1, так что 2D-путь сюда не попадает.
        // …и по НАКОПЛЕННОЙ, когда элемент внутри объёмного контекста
        // (css-transforms-2 §backface-visibility, «m33 < 0 → not rendered»,
        // где m33 — от накопленной: transform3d-backface-visibility-004/006,
        // backface-visibility-hidden-004, backface-visibility-with-sibling-001).
        // `m33` не зависит ни от сдвигов, ни от свёртки `S·M·S⁻¹`, поэтому
        // считается прямо здесь, до перевода в точки устройства.
        let accum33 = match self.under_3d.as_ref().and_then(|f| f.get()) {
            Some((a, _)) => crate::style::computed::mul4(a, self.m4)[2][2],
            None => self.m4[2][2],
        };
        // Владелец `preserve-3d` изнанкой уносит только СЕБЯ: его дети —
        // отдельные плоскости того же контекста и решают свою видимость сами
        // (composited-under-rotateY-180deg-preserve-3d: зелёный ребёнок под
        // `backface-visibility: hidden; rotateY(180deg); preserve-3d`).
        if self.backface_hidden && accum33 < 0.0 && self.frame_3d.is_none() {
            return;
        }
        // Своя `perspective` (css-transforms-2 §perspective-matrix-computation):
        // T(po)·P(d)·T(−po) в точках устройства — в ячейку для детей ДО их
        // отрисовки, на обоих путях. Свёртка та же, что у объёмного пути ниже:
        // сдвиги ×sf, m34 = −1/(d·sf). Сам элемент перспективой не трогается
        // (она действует только на детей) и идёт своим путём как прежде.
        if let (Some(d), Some(frame)) = (self.perspective, self.perspective_frame.as_ref()) {
            use crate::style::computed::{Transform, mul4};
            let px = self
                .perspective_origin_px
                .0
                .unwrap_or(w * self.perspective_origin.0);
            let py = self
                .perspective_origin_px
                .1
                .unwrap_or(h * self.perspective_origin.1);
            let (px_d, py_d) = (
                (f32::from(raw_origin.x) + px) * scale_factor,
                (f32::from(raw_origin.y) + py) * scale_factor,
            );
            let p = mul4(
                mul4(
                    Transform::translate4(px_d, py_d, 0.0),
                    Transform::perspective4(d * scale_factor),
                ),
                Transform::translate4(-px_d, -py_d, 0.0),
            );
            frame.set(Some(p));
        }
        // Плоский путь годится, только когда объёмного контекста рядом нет:
        // и владелец `preserve-3d`, и его ребёнок идут по 4×4, даже когда
        // своих объёмных функций у них нет — `m4` держит и плоские функции
        // (`computed::Transform::m4`, «плоские вкладываются как есть»).
        let in_3d =
            self.frame_3d.is_some() || self.under_3d.as_ref().and_then(|f| f.get()).is_some();
        if !self.has_3d && !in_3d {
            // Плоский путь — прежний по матрице. Маски детей (`overflow`,
            // плитки фона, полосы рамки) едут вместе с содержимым
            // (`Window::with_transformation_masked`): прежде обрезка стояла
            // на месте коробки до `transform` (`transform-clip-001`,
            // `transform-background-001/002`, `transform-fixed-bg-001/003`).
            let quarter = quarter_turn(self.lin)
                .filter(|_| window.current_transformation() == gpui::TransformationMatrix::unit());
            let mut matrix = gpui::TransformationMatrix::unit()
                .translate(origin)
                .compose(gpui::TransformationMatrix {
                    rotation_scale: quarter.unwrap_or(self.lin),
                    translation: [shift(self.tr[0]), shift(self.tr[1])],
                })
                .translate(back);
            let fill_matrix = quarter.map(|_| {
                self.exact_fill_matrix(matrix, bounds.origin, raw_origin, scale_factor)
            });
            if quarter.is_some() {
                // Поворот на кратное четверти (и отражение) оставляет коробку
                // осевой: её края обязаны округляться к точке устройства так
                // же, как у той же коробки, разложенной на месте (чистый сдвиг
                // выше идёт через раскладку — round half up). Растеризатор
                // по правилу «верх-лево» относит ровную половину вниз, а
                // ошибка `f32` у `rotate(-90deg)` (cos ≈ −4e-8) решает
                // ничью случайно — `offset-path-ray-011/013/014` против
                // эталона `translate(...)`. Skia так же кладёт осевой
                // прямоугольник по round(x) (`SkScan::FillRect`), а
                // `gfx::SinCosDegrees` даёт точные 0/±1 у кратных 90°.
                let sf = scale_factor;
                let corner_min = |o: gpui::Point<Pixels>, w: f32, h: f32| {
                    let mut m = (f32::INFINITY, f32::INFINITY);
                    for (dx, dy) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
                        let p = matrix.apply(gpui::point(
                            px((f32::from(o.x) + dx) * sf),
                            px((f32::from(o.y) + dy) * sf),
                        ));
                        m = (m.0.min(f32::from(p.x)), m.1.min(f32::from(p.y)));
                    }
                    m
                };
                let exact_origin = self.exact_origin.unwrap_or(bounds.origin);
                let exact = corner_min(exact_origin, w, h);
                let cur = corner_min(
                    bounds.origin,
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                );
                let snap = |v: f32| ((v * 64.0).round() / 64.0 + 0.5).floor();
                matrix.translation[0] += snap(exact.0) - cur.0;
                matrix.translation[1] += snap(exact.1) - cur.1;
            }
            let child = self.child.as_mut().unwrap();
            window.with_transformation_masked(matrix, |window| {
                if let Some(exact) = fill_matrix {
                    window.with_css_fill_transform(exact, |window| child.paint(window, cx));
                } else {
                    child.paint(window, cx);
                }
            });
            return;
        }
        // --- Объёмный путь: одна 4×4 ОДНОГО элемента, сплющенная на экран ---
        use crate::style::computed::{Transform, det4, mul4};
        let sf = scale_factor;
        // Из css-точек в точки устройства — подобие S·M·S⁻¹, S = diag(sf, sf,
        // sf, 1): столбец сдвига строк 0..2 умножается на sf, строка w
        // столбцов 0..2 делится на sf, m44 НЕ трогается. (В шаге 1 цикл `0..4`
        // домножал и m44 — `flatten_plane` делила на него всю матрицу, и
        // каждый объёмный элемент сжимался в 1/sf; scout-3d-2026-09b.md §1.)
        let mut own = self.m4;
        for i in 0..3 {
            own[i][3] = (own[i][3] + rw * self.m4_pct[i][0] + rh * self.m4_pct[i][1]) * sf;
        }
        for j in 0..3 {
            own[3][j] /= sf;
        }
        // Точка отсчёта по трём осям, в точках устройства: T(o)·M·T(−o).
        // `ox`/`oy` посчитаны выше в css-точках; `ScaledPixels.0` — pub(crate)
        // в gpui, поэтому `origin.x.0` отсюда не читается.
        let oz = self.origin_z.unwrap_or(0.0) * sf;
        let (ox_d, oy_d) = (
            (f32::from(raw_origin.x) + ox) * sf,
            (f32::from(raw_origin.y) + oy) * sf,
        );
        let own = mul4(
            mul4(Transform::translate4(ox_d, oy_d, oz), own),
            Transform::translate4(-ox_d, -oy_d, -oz),
        );
        // Перспектива ПРЯМОГО родителя (§3d-transform-rendering, п.3:
        // «pre-multiply the parent element's perspective matrix»); стека
        // preserve-3d здесь ещё нет — внукам не достаётся
        // (perspective-children-only-*). Ячейку наполнил `paint` родителя в
        // этом же кадре (или прошлом — она переживает кадр), поэтому её видит
        // и отложенный слой абсолюта. Плоский ребёнок (z = 0) под
        // перспективой не меняется — потому только объёмный путь.
        let own = match self.under_perspective.as_ref().and_then(|f| f.get()) {
            Some(p) => mul4(p, own),
            None => own,
        };
        // Вырожденная 4×4 (`scale3d(2, 2, 0)`, transform3d-scale-004:
        // «singular, causes the contents not to display»).
        if det4(&own).abs() < 1e-9 {
            return;
        }
        // Накопленная матрица объёмного контекста (css-transforms-2
        // §accumulated-3d-transformation-matrix): A(родителя) · P(его
        // перспектива — домножена выше) · C(своя). Обе уже в точках
        // устройства, второй свёртки `S·M·S⁻¹` не возникает — ровно из-за
        // неё «3D full» терял −60 (computed.rs:691).
        let under = self.under_3d.as_ref().and_then(|f| f.get());
        let full = match under {
            Some((a, _)) => mul4(a, own),
            None => own,
        };
        let center = (
            (f32::from(raw_origin.x) + w * 0.5) * sf,
            (f32::from(raw_origin.y) + h * 0.5) * sf,
        );
        let flat = flatten_plane(&full, center);
        // Ячейка для СВОИХ детей — накопленная и своя аффинная доля;
        // наполняется ДО отрисовки детей, как у перспективы (и ради
        // отложенных слоёв абсолютов).
        if let Some(frame) = self.frame_3d.as_ref() {
            let share = flat.map_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], |m| {
                [
                    [
                        m.rotation_scale[0][0],
                        m.rotation_scale[0][1],
                        m.translation[0],
                    ],
                    [
                        m.rotation_scale[1][0],
                        m.rotation_scale[1][1],
                        m.translation[1],
                    ],
                ]
            });
            frame.set(Some((full, share)));
        }
        // Plane depth in the 3D rendering context (css-transforms-2
        // §3d-transform-rendering: planes of one context render by z, not
        // document order) — z/w of the box centre under the accumulated
        // matrix; the context root opens the sorting scope.
        let depth = {
            let v = [center.0, center.1, 0.0, 1.0];
            let row = |i: usize| (0..4).map(|k| full[i][k] * v[k]).sum::<f32>();
            let (z, wv) = (row(2), row(3));
            if wv.abs() > 1e-6 { z / wv } else { z }
        };
        let root_3d = self.frame_3d.is_some() && under.is_none();
        let in_context = root_3d || under.is_some();
        let paint_child = |child: &mut AnyElement, m: Option<gpui::TransformationMatrix>, masked: bool, window: &mut Window, cx: &mut App| {
            if in_context {
                let mut body = |window: &mut Window| {
                    window.paint_depth_plane(depth, |window| match m {
                        Some(m) if masked => window.with_transformation_masked(m, |window| child.paint(window, cx)),
                        Some(m) => window.with_transformation(m, |window| child.paint(window, cx)),
                        None => child.paint(window, cx),
                    })
                };
                if root_3d {
                    window.paint_depth_context(body)
                } else {
                    body(window)
                }
            } else {
                match m {
                    Some(m) if masked => window.with_transformation_masked(m, |window| child.paint(window, cx)),
                    Some(m) => window.with_transformation(m, |window| child.paint(window, cx)),
                    None => child.paint(window, cx),
                }
            }
        };
        // Ребро (`rotateX(90deg)`) — не рисуется, как и прежняя нулевая
        // высота. Но в объёмном контексте ПОТОМКИ ребром не становятся
        // (transform3d-preserve3d-011: `rotateX(90)` над `rotateX(90)` =
        // 180°): краска идёт под единичной долей, а место каждый потомок
        // назначает себе сам по накопленной.
        let Some(flat) = flat else {
            if self.frame_3d.is_some() {
                let child = self.child.as_mut().unwrap();
                paint_child(child, Some(gpui::TransformationMatrix::unit()), false, window, cx);
            }
            return;
        };
        // Своя доля для gpui: родитель УЖЕ втолкнул `F_P`, а вложения
        // складываются как `inner∘outer` (`window.rs:2789`, порядок замерен и
        // оставлен) — значит втолкнуть надо `G ∘ F_P⁻¹`.
        let flat = match under.and_then(|(_, fp)| invert_affine(fp)) {
            Some(inv) => flat.compose(inv),
            None => flat,
        };
        let child = self.child.as_mut().unwrap();
        // Маски детей едут за сплющенной матрицей (как на плоском пути,
        // `Window::with_transformation_masked`); косая — прежнее поведение.
        paint_child(child, Some(flat), true, window, cx);
    }
}

impl IntoElement for Transformed {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
