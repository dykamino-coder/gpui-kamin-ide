//! Результаты оформления: цвет краски, активный фильтр и операнды фона.
//! Filter Effects 1 §5 обрабатывает краску; наследование CSS не читает этот результат.

use super::background_paint::BackgroundValues;
use super::computed::{Computed, Filter, Shadow};
use super::values::value::{Color, StyleColor};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForegroundPaint {
    pub color: Option<Color>,
    pub raw: Option<Color>,
    pub fill: Option<Color>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecorationPaint {
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
    pub fn from_computed(c: &Computed) -> Self {
        Self {
            decorations: true,
            preparation: None,
            foreground: ForegroundPaint {
                color: c.inh.ui.color,
                ..Default::default()
            },
            filter: c.reset_effects.filter,
            filter_ops: c.reset_effects.scrollbar_filter_ops.clone(),
            background: BackgroundValues::from_computed(c),
            border_colors: c.reset_border.border_colors.clone(),
            shadows: c.reset_effects.shadows.clone(),
        }
    }
}

impl DecorationPaint {
    /// Whether the paint result paints like `other`: equal but for the inks
    /// (`foreground`, those of `preparation`), which no border side or shadow
    /// of the box of style `style` resolves as `currentcolor`.
    pub fn same_unread_color(&self, other: &Self, style: &Computed) -> bool {
        let Self {
            decorations,
            preparation,
            foreground: _,
            filter,
            filter_ops,
            background,
            border_colors,
            shadows,
        } = self;
        let preparation = match (preparation, &other.preparation) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                let PaintInput {
                    parent: _,
                    foreground: _,
                    element,
                    declares_color,
                } = a;
                *element == b.element && *declares_color == b.declares_color
            }
            _ => false,
        };
        *decorations == other.decorations
            && preparation
            && *filter == other.filter
            && *filter_ops == other.filter_ops
            && *background == other.background
            && *border_colors == other.border_colors
            && *shadows == other.shadows
            && !self.reads_color(style)
    }

    /// Whether a border side or a shadow of the result resolves the ink of
    /// the box of style `style` as `currentcolor`.
    pub fn reads_color(&self, style: &Computed) -> bool {
        style.border_reads_color(&self.border_colors)
            || self.shadows.iter().any(|s| s.color.depends_on_current())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintInput {
    pub parent: ForegroundPaint,
    pub foreground: ForegroundPaint,
    pub element: bool,
    pub declares_color: bool,
}
