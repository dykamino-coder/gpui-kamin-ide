//! Нижний типизированный вход существующего моста синтетических стилей:
//! значения коробки и причина, по которой коробку создала раскладка.

use crate::box_facts::BoxContext;
use crate::style::computed::Align;
use crate::style::computed::writing_mode::WritingMode;

/// Типизированный вход существующего layout-made моста. A07 удаляет транспорт
/// вместе с `layout_made`; вычисленные CSS-значения не владеют этими фактами.
#[derive(Clone, Debug)]
pub struct BoxStyleInput {
    pub(crate) style: std::rc::Rc<crate::style::computed::Computed>,
    pub(crate) context: Option<std::rc::Rc<BoxContext<WritingMode, Align>>>,
    pub(crate) canvas_background: Option<crate::style::canvas_background::CanvasBackgroundSource>,
    pub(crate) background_paint: crate::style::background_paint::BackgroundPaint,
    pub(crate) inline_decoration: crate::style::inline_decoration::InlineDecoration,
    /// The element style whose cascade `position-try` variants recompute,
    /// when the box values come from a box stage of that element.
    pub(crate) try_source: Option<crate::dom::SharedStyle>,
    /// The colours already passed the subtree colour filter of the stage
    /// that produced them (Filter Effects 1 §5 paints a subtree once).
    pub(crate) filter_applied: bool,
}

impl BoxStyleInput {
    pub(crate) fn new(
        style: std::rc::Rc<crate::style::computed::Computed>,
        context: Option<std::rc::Rc<BoxContext<WritingMode, Align>>>,
        inline_decoration: crate::style::inline_decoration::InlineDecoration,
        background_paint: crate::style::background_paint::BackgroundPaint,
        canvas_background: Option<crate::style::canvas_background::CanvasBackgroundSource>,
    ) -> Self {
        Self {
            style,
            context,
            inline_decoration,
            background_paint,
            canvas_background,
            try_source: None,
            filter_applied: false,
        }
    }
}

impl From<std::rc::Rc<crate::style::computed::Computed>> for BoxStyleInput {
    fn from(style: std::rc::Rc<crate::style::computed::Computed>) -> Self {
        Self::new(style, None, Default::default(), Default::default(), None)
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

/// Why layout, not the element tree's box generation, made a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoxOrigin {
    /// An element's own box that layout placed under another box parent
    /// (a hoisted absolutely positioned box, a float lifted out of an inline).
    /// The element stays its own CSS inheritance parent's child (CSS Cascade 5
    /// §7.2, CSS 2 §9.5 and §10.1).
    Moved,
    /// The generated `::marker` box of a list item (CSS Lists 3 §3.1,
    /// CSS Pseudo 4 §3.3).
    Marker,
    /// The CSS box of `foreignObject` laid out as the root of its embedded
    /// HTML viewport, without the host's SVG effects (SVG 2 Embedded Content,
    /// the `foreignObject` element).
    EmbeddedRoot,
}

/// The box input of a layout-made box and the reason layout made it. Its
/// computed values were resolved under the box's CSS parent box; the element
/// that carries it re-enters box construction under its new box parent.
#[derive(Clone, Debug)]
pub(crate) struct LayoutBoxInput {
    pub(crate) origin: BoxOrigin,
    pub(crate) style: BoxStyleInput,
}

impl LayoutBoxInput {
    pub(crate) fn new(origin: BoxOrigin, style: impl Into<BoxStyleInput>) -> Self {
        Self {
            origin,
            style: style.into(),
        }
    }
}
