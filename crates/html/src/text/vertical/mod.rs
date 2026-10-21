//! Вертикальное письмо (`writing-mode: vertical-*`, `sideways-*`): абзац набирается
//! горизонтально и поворачивается на четверть при отрисовке (`VerticalText`), составной знак
//! `text-combine-upright` (`CombinedUpright`), левый край коробки корня для фона холста.
//! Состояние принадлежит документу: запасной двухкадровый замер повёрнутых атомов
//! (`VerticalMeasures`), рамка поворота на подготовке (`VerticalPaint`, `TurnFrame`;
//! отображает до-поворотную точку статической позиции в экранную плоскость) и протяжённость
//! строки для внутренней меры (`InlineMeasure`).
mod combined_upright;
pub(crate) mod measurements;
mod root_left;
mod vertical_text;
pub use crate::text::vertical::combined_upright::CombinedUpright;
pub use crate::text::vertical::root_left::record_root_left;
pub use crate::text::vertical::vertical_text::VerticalText;

mod combined_geometry;
pub(crate) mod combined_text;
mod vertical_line_baseline;
mod vertical_style;

mod paint_context;
pub(crate) use measurements::VerticalMeasures;
pub(crate) use paint_context::{TurnFrame, VerticalPaint};

mod inline_measure;
pub use inline_measure::InlineMeasure;
