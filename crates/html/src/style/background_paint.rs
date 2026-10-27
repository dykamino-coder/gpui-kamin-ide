//! Собственный вход фона коробки, отдельно от вычисленных CSS-значений.

mod gradient;
mod values;
pub(crate) use values::BackgroundValues;

/// css-backgrounds-3 §2.11.2: после распространения фона body только его
/// used-значения становятся initial. Computed остаются источником inherit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BackgroundPaint {
    #[default]
    Css,
    Initial,
}

impl BackgroundPaint {
    pub(crate) fn paints(self) -> bool {
        self == Self::Css
    }
}

/// Не наследуемые входы оформления в существующем транспорте коробки.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DecorationInput {
    pub inline: super::inline_decoration::InlineDecoration,
    pub background_paint: BackgroundPaint,
    pub paint: Option<std::rc::Rc<super::decoration_paint::DecorationPaint>>,
    pub canvas: Option<super::canvas_background::CanvasBackgroundSource>,
}
