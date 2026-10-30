//! Фрагментация: формы, полосы, переносимые коробки.

pub(crate) mod axis_slice;
pub mod breaks;
pub mod clone;
pub mod flex_lines;
pub(crate) mod flow;
pub(crate) mod geometry;
pub mod grid_bands;
pub(crate) mod line_admission;
pub mod line_shape;
pub(crate) mod paginate;
pub(crate) mod plan;
pub mod probe;
pub mod push;
pub(crate) mod replay;
pub(crate) mod replay_pieces;
pub(crate) mod static_coordinates;
pub mod table_bands;
// Модель токенов вводится раньше потребителей: укладка читает из неё
// `Space`, остальное подключают шаги перехода на токены.
pub(crate) mod decoration;
pub(crate) mod paint_source;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "break-token model lands before its consumers")
)]
pub(crate) mod token;
pub mod types;

pub(crate) mod effect_fragment;
pub(super) mod fragment_mask;
pub(crate) mod gap_fragment;
mod row_element;

mod flow_margins;
pub(crate) use flow_margins::FlowMargins;

/// Условия записи разрывов: `paged` — стопка страниц (монолитом считается и
/// `contain: size`, см. `solid_box`); `viewport` — размер области просмотра
/// для `vh`/`vw`: у страниц это page area, у колонок единицы окна остаются
/// неразрешёнными.
#[derive(Clone, Copy)]
pub(crate) struct ShapeCx {
    pub(crate) paged: bool,
    pub(crate) viewport: Option<(f32, f32)>,
}

impl ShapeCx {
    pub(crate) const COLUMNS: Self = Self {
        paged: false,
        viewport: None,
    };
}

mod line_context;
pub(crate) use line_context::{LineCx, with_lines};
