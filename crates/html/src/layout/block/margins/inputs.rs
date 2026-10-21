//! Входы раннего разрешения длин полей, до наследования стиля потомками.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CollapseInputs {
    pub font: f32,
    pub width: Option<f32>,
    pub height_def: bool,
}

impl Default for CollapseInputs {
    fn default() -> Self {
        Self {
            font: 16.0,
            width: None,
            height_def: false,
        }
    }
}
