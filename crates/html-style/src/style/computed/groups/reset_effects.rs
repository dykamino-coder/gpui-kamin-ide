//! CSS-операнды группы effects; значение за общим COW-указателем.
use super::super::*;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetEffects {
    pub opacity: Option<f32>,
    /// `box-shadow: inherit`.
    pub shadow_inherit: bool,
    pub shadows: Vec<Shadow>,
    /// Внутренние тени (`box-shadow: inset`) — отдельным списком: рисуются
    /// поверх заливки, а не под фигурой.
    pub inset_shadows: Vec<Shadow>,
    /// `backdrop-filter: blur(N)` — размытие того, что под элементом.
    pub backdrop_blur: Option<f32>,
    /// `backdrop-filter`: цветовые функции списка (без размытия). Рисуются
    /// матрицей 4×5 тем же проходом подложки (`Filter::color_matrix`).
    pub backdrop_color: Option<Filter>,
    /// `box-shadow` с длинами в единицах шрифта — так же ждёт своего кегля и
    /// разбирается в `resolve_em` (`box-shadow-calc`: `calc(1em + 10px)`
    /// прежде ронял тень целиком — `parse_shadows` пропускает `em`).
    pub shadow_raw: Option<String>,
    /// `mix-blend-mode`: как слой смешивается с тем, что под ним.
    pub blend: Option<u8>,
    /// `isolation: isolate`: поддерево смешивается внутри себя, а с кадром —
    /// уже готовой картинкой.
    pub isolate: Option<bool>,
    /// `will-change` (css-will-change-1): разряды `wc::*`; ноль — `auto`.
    /// Слоёв композитора у нас нет, поэтому от обещания остаётся ровно то,
    /// что дало бы само свойство: содержащий блок и контекст наложения.
    pub will_change: u8,
    /// Есть ли выше по дереву корень подложки (filter-effects-2
    /// §BackdropRoot): прозрачность, фильтр, маска, clip-path, смешивание,
    /// `backdrop-filter`, `will-change` с ними. Ставится при наследовании.
    pub backdrop_root_above: bool,
    /// `will-change` называет свойство, создающее корень подложки.
    pub will_change_root: bool,
    /// `backdrop-filter` задан (не `none`): корень подложки при ЛЮБОМ списке,
    /// даже тождественном `invert(0)`, у которого матрицы нет
    /// (filter-effects-2 Overview.bs:119; Blink
    /// paint_property_tree_builder.cc:1846 `!BackdropFilter().IsEmpty()`).
    pub backdrop_filter_set: bool,
    /// `view-transition-name` не `none` — тоже корень подложки
    /// (css-view-transitions-1 Overview.bs:582).
    pub vt_name: bool,
    /// `backdrop-filter: url(#id)` — ссылка на SVG `<filter>`; сводится к
    /// матрице 4×5 на отрисовке (`render::svg_filter_matrix`).
    pub backdrop_ref: Option<String>,
    /// `filter`: цветовые преобразования, применённые к собственным цветам.
    pub filter: Option<Filter>,
    /// Ordered color operations for platform scrollbar primitives.
    pub scrollbar_filter_ops: Vec<Filter>,
    /// `filter: url(#id)` — ссылка на SVG-`<filter>` документа; рисуется
    /// растровым слоем поверх коробки (`paint::effects::filter::FilterLayer`).
    pub filter_ref: Option<String>,
    /// `filter: drop-shadow(...)` — тень фильтра; у коробки со сплошным
    /// фоном становится внешней тенью (`style::cascade::inherit`).
    pub drop_shadow: Option<Shadow>,
}
