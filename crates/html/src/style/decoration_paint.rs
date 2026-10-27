//! Результаты оформления: цвет краски, активный фильтр и операнды фона.
//! Filter Effects 1 §5 обрабатывает краску; наследование CSS не читает этот результат.

use super::background_paint::BackgroundValues;
use super::computed::{Computed, Filter, Shadow};
use super::values::value::{Color, StyleColor};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ForegroundPaint {
    pub color: Option<Color>,
    pub raw: Option<Color>,
    pub fill: Option<Color>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DecorationPaint {
    pub decorations: bool,
    /// Подъём коробки до paint сохраняет входы исходного родителя; готовый
    /// результат переносится без повторной окраски (Filter Effects 1 §5).
    pub preparation: Option<PaintInput>,
    pub foreground: ForegroundPaint,
    pub filter: Option<Filter>,
    pub filter_ops: Vec<Filter>,
    pub background: BackgroundValues,
    pub border_colors: [StyleColor; 4],
    pub shadows: Vec<Shadow>,
}

impl DecorationPaint {
    pub(crate) fn from_computed(c: &Computed) -> Self {
        Self {
            decorations: true,
            preparation: None,
            foreground: ForegroundPaint {
                color: c.color,
                ..Default::default()
            },
            filter: c.filter,
            filter_ops: c.scrollbar_filter_ops.clone(),
            background: BackgroundValues::from_computed(c),
            border_colors: c.border_colors.clone(),
            shadows: c.shadows.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaintInput {
    pub parent: ForegroundPaint,
    pub foreground: ForegroundPaint,
    pub element: bool,
    pub declares_color: bool,
}
