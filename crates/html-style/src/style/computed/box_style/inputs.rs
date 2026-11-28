//! Производители входов сборки коробки; нейтральные данные живут в box_facts.

use crate::box_facts::ApplicabilityInputs;

use crate::style::computed::{BoxStyle, Display as D};

impl BoxStyle {
    pub fn applicability_inputs(&self) -> ApplicabilityInputs {
        ApplicabilityInputs {
            flex: matches!(self.reset_box_layout.display, Some(D::Flex | D::InlineFlex)),
            rtl: self.wm().rtl(),
            wrap_reverse: self.reset_flex.flex_wrap_reverse == Some(true),
        }
    }
}

impl From<&BoxStyle> for crate::style::box_input::BoxStyleInput {
    fn from(style: &BoxStyle) -> Self {
        Self {
            try_source: style.try_style.clone(),
            decor_box: style.decor_box,
            flow_align: style.flow_align,
            tree: style.tree,
            decors: style.decors.clone(),
            ..Self::new(
                std::rc::Rc::clone(style.style_rc()),
                Some(std::rc::Rc::new(style.context.clone())),
                style.inline_decoration,
                style.background_paint,
                style.canvas_background.clone(),
                style.paint.clone(),
            )
        }
    }
}

impl From<BoxStyle> for crate::style::box_input::BoxStyleInput {
    fn from(style: BoxStyle) -> Self {
        let context = std::rc::Rc::new(style.context.clone());
        let inline_decoration = style.inline_decoration;
        Self {
            try_source: style.try_style,
            decor_box: style.decor_box,
            flow_align: style.flow_align,
            tree: style.tree,
            decors: style.decors,
            ..Self::new(
                style.style,
                Some(context),
                inline_decoration,
                style.background_paint,
                style.canvas_background,
                style.paint,
            )
        }
    }
}
