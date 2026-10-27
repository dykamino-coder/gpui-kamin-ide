//! Вычисленные фоновые операнды донора холста без изменения CSS корня.

pub(crate) use super::background_paint::BackgroundValues as CanvasBackground;

/// Собственный источник фона холста; роль не наследуется как CSS.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasBackgroundSource {
    Root,
    Donor(std::rc::Rc<CanvasBackground>),
}
