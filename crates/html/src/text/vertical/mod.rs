//! Вертикальный текст.
mod combined_upright;
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
pub(crate) use paint_context::{TurnFrame, VerticalPaint};

mod measures;
pub(crate) use measures::VerticalMeasures;

/// Stable occurrence key within this document's current traversal.
pub fn vt_seq_key(base: u64, vcx: crate::document::state::ValueCx<'_>) -> u64 {
    vcx.doc.vertical_measures.sequence_key(base)
}

thread_local! {
    /// Сборщик внутреннего строчного размера повёрнутого текста (`None` —
    /// закрыт): наибольшая длина строки всех `VerticalText`, разложенных,
    /// пока он открыт.
    pub static VT_INLINE_MAX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}
