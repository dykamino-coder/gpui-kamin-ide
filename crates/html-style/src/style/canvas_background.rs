//! Вычисленные фоновые операнды донора холста без изменения CSS корня.

pub use super::background_paint::BackgroundValues as CanvasBackground;

/// Собственный источник фона холста; роль не наследуется как CSS.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasBackgroundSource {
    Root,
    Donor(std::rc::Rc<CanvasBackground>),
}
