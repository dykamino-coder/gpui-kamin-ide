//! Собственный вход фона коробки, отдельно от вычисленных CSS-значений.

mod gradient;
mod values;
pub use values::BackgroundValues;

/// css-backgrounds-3 §2.11.2: после распространения фона body только его
/// used-значения становятся initial. Computed остаются источником inherit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackgroundPaint {
    #[default]
    Css,
    Initial,
}

impl BackgroundPaint {
    pub fn paints(self) -> bool {
        self == Self::Css
    }
}

/// Не наследуемые входы оформления в существующем транспорте коробки.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecorationInput {
    pub inline: super::inline_decoration::InlineDecoration,
    pub background_paint: BackgroundPaint,
    pub paint: Option<std::rc::Rc<super::decoration_paint::DecorationPaint>>,
    pub canvas: Option<super::canvas_background::CanvasBackgroundSource>,
}

impl DecorationInput {
    /// Whether layout and ink extents read the same values from both inputs:
    /// equal but for colours, background values and the canvas source, which
    /// only paint reads (`Computed::same_layout`). Inline padding and border
    /// widths, text decorations, filters and shadows compare exactly.
    pub fn same_layout(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        let Self {
            inline,
            background_paint: _,
            paint,
            canvas: _,
        } = self;
        let super::inline_decoration::InlineDecoration {
            band_color: _,
            border_stroke,
            physical_padding,
            corner_radius: _,
        } = inline;
        let paint = match (paint, &other.paint) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                let super::decoration_paint::DecorationPaint {
                    decorations,
                    preparation: _,
                    foreground: _,
                    filter,
                    filter_ops,
                    background: _,
                    border_colors: _,
                    shadows,
                } = &**a;
                std::rc::Rc::ptr_eq(a, b)
                    || *decorations == b.decorations
                        && *filter == b.filter
                        && *filter_ops == b.filter_ops
                        && *shadows == b.shadows
            }
            _ => false,
        };
        paint
            && *physical_padding == other.inline.physical_padding
            && border_stroke.map(|(_, widths)| widths)
                == other.inline.border_stroke.map(|(_, widths)| widths)
    }
}
