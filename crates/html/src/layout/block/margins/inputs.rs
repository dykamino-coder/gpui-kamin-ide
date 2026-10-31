//! Входы раннего разрешения длин полей, до наследования стиля потомками, и контекст спуска
//! схлопывания полей (`MarginCx`): значения документа плюс входы текущего уровня.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
use crate::values::ValueCx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CollapseInputs {
    pub font: f32,
    pub width: Option<f32>,
    pub height_def: bool,
}

impl Default for CollapseInputs {
    fn default() -> Self {
        Self {
            font: MEDIUM_FONT_SIZE_PX,
            width: None,
            height_def: false,
        }
    }
}

/// Context of a margin-collapse descent: the document's values and the level inputs
/// (`CollapseInputs`) that lengths of the current level resolve against. Calls into
/// style and text receive `values`; only the margin algorithms read `collapse`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MarginCx<'a> {
    pub(crate) values: ValueCx<'a>,
    pub(crate) collapse: CollapseInputs,
}

impl<'a> MarginCx<'a> {
    /// CSS2 §9.5.2: проверять возможность clearance, сохраняя свойство CSS.
    pub(crate) fn clear(self, style: &crate::style::computed::Computed) -> Option<i8> {
        crate::layout::float::clear::effective_clear(style, self.values)
    }

    /// Outside any descent: the default level (medium font, no containing-block width,
    /// indefinite height).
    pub(crate) fn new(values: ValueCx<'a>) -> Self {
        Self {
            values,
            collapse: CollapseInputs::default(),
        }
    }

    /// The same descent with the inputs of another level.
    pub(crate) fn with_collapse(mut self, collapse: CollapseInputs) -> Self {
        self.collapse = collapse;
        self
    }
}
