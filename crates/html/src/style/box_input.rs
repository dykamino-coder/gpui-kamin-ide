//! Нижний типизированный вход существующего моста синтетических стилей.

use crate::box_facts::BoxContext;
use crate::style::computed::Align;
use crate::style::computed::writing_mode::WritingMode;

/// Типизированный вход существующего layout-made моста. A07 удаляет транспорт
/// вместе с `layout_made`; вычисленные CSS-значения не владеют этими фактами.
#[derive(Clone, Debug)]
pub struct BoxStyleInput {
    pub(crate) style: std::rc::Rc<crate::style::computed::Computed>,
    pub(crate) context: Option<std::rc::Rc<BoxContext<WritingMode, Align>>>,
}

impl BoxStyleInput {
    pub(crate) fn new(
        style: std::rc::Rc<crate::style::computed::Computed>,
        context: Option<std::rc::Rc<BoxContext<WritingMode, Align>>>,
    ) -> Self {
        Self { style, context }
    }
}

impl From<std::rc::Rc<crate::style::computed::Computed>> for BoxStyleInput {
    fn from(style: std::rc::Rc<crate::style::computed::Computed>) -> Self {
        Self::new(style, None)
    }
}

impl From<&BoxStyleInput> for BoxStyleInput {
    fn from(input: &BoxStyleInput) -> Self {
        input.clone()
    }
}

impl std::ops::Deref for BoxStyleInput {
    type Target = std::rc::Rc<crate::style::computed::Computed>;

    fn deref(&self) -> &Self::Target {
        &self.style
    }
}
