//! Фрагментация: формы, полосы, переносимые коробки.

pub mod breaks;
pub mod clone;
pub mod flex_lines;
pub(crate) mod flow;
pub mod grid_bands;
pub mod line_shape;
pub(crate) mod oof_reach;
pub(crate) mod paginate;
pub(crate) mod plan;
pub mod probe;
pub mod push;
pub mod shape_contents;
mod shape_kids;
pub(crate) mod static_coordinates;
pub mod table_bands;
pub mod types;

pub(crate) mod effect_fragment;
pub(super) mod fragment_mask;
pub(crate) mod fragment_size;
pub(crate) mod gap_fragment;
pub(super) mod intrinsic_measure;
mod row_element;

/// Элемент сетки для `grid_auto_row_bands`: номер среди `c.children`, ряд,
/// верхнее поле, мера `shape_full`, годны ли его внутренние точки сетке.
type GridSpot = (usize, usize, f32, Shape, bool);

mod shape;
pub(crate) use shape::Shape;

/// Условия меры: `paged` — стопка страниц (монолитом считается и
/// `contain: size`, см. `solid_box`); `viewport` — размер области просмотра
/// для `vh`/`vw`: у страниц это page area, у колонок единицы окна остаются
/// неразрешёнными (`None` → отказ от меры, прежнее поведение).
#[derive(Clone, Copy)]
pub(crate) struct ShapeCx {
    pub(crate) paged: bool,
    /// Nested fragmentainers and cloned decorations retain their own model.
    pub(crate) legacy_oof: bool,
    pub(crate) viewport: Option<(f32, f32)>,
    /// Мера ПОТОКА, а не коробки: заданная высота не обрезает ни высоту, ни
    /// точки разреза, ни монолитные диапазоны. Переполнение коробки с
    /// заданной высотой — параллельный поток (css-break-3 §3), и укладке
    /// колонок нужна его протяжённость ОТДЕЛЬНО от высоты коробки.
    /// Бюджет ОДИН на путь: флаг гаснет на первой же коробке с заданной
    /// высотой (`shape_full`, перепривязка `cx`). Вложенная ограниченная
    /// коробка заводит СВОЙ параллельный поток, в поток предка он не
    /// входит, а модель несёт один `over` на ребёнка стопки — второго
    /// потока ей выразить нечем. Через коробки с высотой `auto` флаг идёт
    /// насквозь: там своей ограниченности нет
    /// (`overflowed-block-with-room-after-003` — обёртка `auto` над
    /// коробкой 70).
    pub(crate) unclamped: bool,
}

impl ShapeCx {
    pub(crate) const COLUMNS: ShapeCx = ShapeCx {
        paged: false,
        legacy_oof: false,
        viewport: None,
        unclamped: false,
    };
}

mod line_context;
pub(crate) use line_context::{LineCx, LineFrame, with_lines};
