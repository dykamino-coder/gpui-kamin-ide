//! Вертикальный текст.
mod combined_upright;
pub(crate) mod measurements;
mod root_left;
mod vertical_text;
pub use crate::text::vertical::combined_upright::CombinedUpright;
pub use crate::text::vertical::root_left::record_root_left;
pub use crate::text::vertical::vertical_text::VerticalText;

// owner: A

mod combined_geometry;
pub(crate) mod combined_text;
mod vertical_line_baseline;
mod vertical_style;

mod paint_context;
pub(crate) use measurements::VerticalMeasures;
pub(crate) use paint_context::{TurnFrame, VerticalPaint};

mod inline_measure;
pub use inline_measure::InlineMeasure;
