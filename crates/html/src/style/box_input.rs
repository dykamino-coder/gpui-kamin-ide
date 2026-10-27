//! Нижний типизированный вход существующего моста синтетических стилей.

use crate::box_facts::BoxContext;
use crate::style::computed::Align;
use crate::style::computed::writing_mode::WritingMode;

/// Типизированный вход существующего layout-made моста. A07 удаляет транспорт
/// вместе с `layout_made`; вычисленные CSS-значения не владеют этими фактами.
#[derive(Clone, Debug)]
pub struct BoxStyleInput {
    pub(crate) paint: Option<std::rc::Rc<crate::style::decoration_paint::DecorationPaint>>,
    pub(crate) style: std::rc::Rc<crate::style::computed::Computed>,
    pub(crate) context: Option<std::rc::Rc<BoxContext<WritingMode, Align>>>,
    pub(crate) canvas_background: Option<crate::style::canvas_background::CanvasBackgroundSource>,
    pub(crate) background_paint: crate::style::background_paint::BackgroundPaint,
    pub(crate) inline_decoration: crate::style::inline_decoration::InlineDecoration,
}

impl BoxStyleInput {
    pub(crate) fn new(
        style: std::rc::Rc<crate::style::computed::Computed>,
        context: Option<std::rc::Rc<BoxContext<WritingMode, Align>>>,
        inline_decoration: crate::style::inline_decoration::InlineDecoration,
        background_paint: crate::style::background_paint::BackgroundPaint,
        canvas_background: Option<crate::style::canvas_background::CanvasBackgroundSource>,
        paint: Option<std::rc::Rc<crate::style::decoration_paint::DecorationPaint>>,
    ) -> Self {
        Self {
            style,
            paint,
            context,
            inline_decoration,
            background_paint,
            canvas_background,
        }
    }
}

impl From<std::rc::Rc<crate::style::computed::Computed>> for BoxStyleInput {
    fn from(style: std::rc::Rc<crate::style::computed::Computed>) -> Self {
        Self::new(
            style,
            None,
            Default::default(),
            Default::default(),
            None,
            None,
        )
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
