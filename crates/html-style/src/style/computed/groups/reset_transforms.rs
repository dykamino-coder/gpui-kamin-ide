//! CSS-операнды группы transforms; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetTransforms {
    /// `translate` — визуальный сдвиг, не меняющий раскладку.
    pub translate: Option<(Len, Len)>,
    /// `transform`/`rotate`/`scale`: поворот в радианах и масштаб по осям.
    pub transform: Option<Transform>,
    /// `rotate` вокруг оси z (радианы) и `scale` по осям — отдельными полями:
    /// разложение в `transform` порядок теряет, а css-transforms-2 §ctm ставит
    /// их СЛЕВА от списка `transform` (п.4-5 перед п.7). `None` — `none`.
    pub rotate_prop: Option<f32>,
    pub scale_prop: Option<(f32, f32)>,
    /// `offset-path` как записано (motion-1 §offset-path): `path('…')`,
    /// `ray(…)`, `<basic-shape>` или `url(#id)`. Разбирается не здесь:
    /// сэмплеру нужны все остальные `offset-*`, а каскад сводит их вразнобой.
    pub offset_path: Option<String>,
    /// `offset-distance`: длина или доля ДЛИНЫ ПУТИ (а не коробки).
    pub offset_distance: Option<Len>,
    /// `offset-rotate` как записано: `auto | reverse | <angle> | auto <angle>`.
    pub offset_rotate: Option<String>,
    /// `offset-anchor` как записано; `auto` — это точка `transform-origin`.
    pub offset_anchor: Option<String>,
    /// `offset-position` как записано: `normal | auto | <position>`.
    pub offset_position: Option<String>,
    /// Offset-трансформ откладывается до отрисовки: содержащий блок элемента —
    /// начальный (его размер знает только сборщик, `animation::motion`).
    pub offset_icb: bool,
    /// `backface-visibility: hidden`.
    pub backface_hidden: Option<bool>,
    /// `transform-origin` в долях размера элемента.
    pub transform_origin: Option<(f32, f32)>,
    /// `transform`/`transform-origin` с длинами в единицах шрифта: запись
    /// ждёт своего кегля и разбирается в `resolve_em` (css-transforms-1
    /// §computed value: относительные длины становятся абсолютными).
    pub transform_raw: Option<String>,
    /// Есть ли выше трансформированный предок: он — содержащий блок и для
    /// `position: fixed` (css-transforms-1 §transform-rendering: «…for all
    /// of its absolute-position descendants, fixed-position descendants»).
    pub transform_ancestor: bool,
    /// Transformed ancestor in the strict css-transforms-1 sense (a transform,
    /// not containment or filters): `background-attachment: fixed` below it
    /// behaves as `scroll` (css-transforms-1 §6 Transform Rendering Model).
    pub transformed_ancestor: bool,
    pub transform_origin_raw: Option<String>,
    /// Точка отсчёта преобразования В ТОЧКАХ по осям — когда записана длиной,
    /// а не долей. Долю из неё делает отрисовка: размер коробки известен там.
    pub transform_origin_px: (Option<f32>, Option<f32>),
    /// Третья координата `transform-origin` в точках (css-transforms-2);
    /// на плоскую матрицу не влияет, на 4×4 — `T(o)·M·T(−o)` по трём осям.
    pub transform_origin_z: Option<f32>,
    /// `perspective` (css-transforms-2 §perspective-property) — расстояние
    /// до глаза в css-точках для ОБЪЁМНЫХ ДЕТЕЙ, уже не меньше 1px («values
    /// less than 1px must be treated as 1px»); `none` = None.
    pub perspective: Option<f32>,
    /// `perspective-origin` долями коробки (умолчание 50% 50%) и в точках по
    /// осям, когда записан длиной — как `transform_origin`/`_px`.
    pub perspective_origin: Option<(f32, f32)>,
    pub perspective_origin_px: (Option<f32>, Option<f32>),
    /// `transform-box: fill-box` у SVG-фигуры (css-transforms-1
    /// §transform-box): длины `transform-origin` отсчитываются от рамки
    /// фигуры, а не от вьюпорта.
    pub transform_box_fill: Option<bool>,
    /// Опорная коробка `transform-box` целиком (css-transforms-1
    /// §transform-box) — для SVG-элементов без CSS-коробки: 0 — `view-box`,
    /// 1 — `fill-box` (и `content-box`: «the used value for content-box is
    /// fill-box»), 2 — `stroke-box` (и `border-box`). None — не задано.
    pub transform_box: Option<u8>,
    /// Ячейка матрицы перспективы (см. `PerspectiveFrame`); один и тот же
    /// `Rc` у `e.style` родителя, его `merged` и `inherited` детей.
    pub perspective_frame: Option<PerspectiveFrame>,
    /// `transform-style: preserve-3d` — элемент образует объёмный контекст
    /// (css-transforms-2 §transform-style-property). ИСПОЛЬЗУЕМОЕ значение
    /// гасят «групповые» свойства — это решает `render::flattens_3d`,
    /// потому что они могут быть записаны в блоке ПОСЛЕ `transform-style`.
    pub preserve_3d: Option<bool>,
    /// Ячейка накопленной 4×4 (см. `Frame3d`): заводится при разборе
    /// `transform-style`, наполняется `Transformed::paint` владельца,
    /// читается объёмным путём ПРЯМЫХ детей.
    pub frame_3d: Option<Frame3d>,
}
