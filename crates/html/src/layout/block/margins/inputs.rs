//! Входы раннего разрешения длин полей, до наследования стиля потомками.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
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
