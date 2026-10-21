//! Записи временного layout-движка для измерения clamp внутри хоста.
//! LayoutId действителен только до освобождения этого движка.

use super::ClampEntry;
use crate::layout::probe::Sink;
use gpui::LayoutId;

type MeasuredRows = (u32, Vec<(f32, f32)>);

#[derive(Clone, Default)]
#[allow(dead_code)] // Носитель для хоста; текущие пробы остаются prepaint-пробами.
pub(crate) struct ClampMeasure {
    pub(crate) entries: Sink<(LayoutId, ClampEntry)>,
    pub(crate) rows: Sink<MeasuredRows>,
}
