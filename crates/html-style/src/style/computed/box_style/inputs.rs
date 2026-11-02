//! Производители входов сборки коробки; нейтральные данные живут в box_facts.

use crate::box_facts::{ApplicabilityInputs, DecorationInputs};

use crate::style::computed::{BoxStyle, Computed, Decor, Display as D, Position as P};

impl BoxStyle {
    pub fn applicability_inputs(&self) -> ApplicabilityInputs {
        ApplicabilityInputs {
            flex: matches!(self.reset_box_layout.display, Some(D::Flex | D::InlineFlex)),
            rtl: self.wm().rtl(),
            wrap_reverse: self.reset_flex.flex_wrap_reverse == Some(true),
        }
    }

    pub fn decoration_inputs(&self, own: &Computed) -> DecorationInputs<'_, Decor> {
        // Сохраняем тест применимости стадии коробки. Преобразование display
        // остаётся в вычисленной стадии (CSS Display 3 §2.7).
        let blockified = matches!(
            self.reset_box_layout.display,
            Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid | D::GridLanes)
        );
        let blocked = matches!(own.reset_positioned.position, Some(P::Absolute | P::Fixed))
            || own.reset_box_layout.float.is_some_and(|f| f != 0)
            || (!blockified
                && matches!(
                    own.reset_box_layout.display,
                    Some(D::InlineBlock | D::InlineTable | D::InlineFlex | D::InlineGrid)
                ));
        DecorationInputs {
            inherited: if blocked {
                &[]
            } else {
                &self.reset_text_box.decors
            },
            block: match own.reset_box_layout.display {
                None => own.block_tag && own.reset_context.inline_display != Some(true),
                Some(D::Contents) => false,
                Some(_) => true,
            },
            generates_box: own.reset_box_layout.display != Some(D::Contents),
        }
    }
}

impl From<&BoxStyle> for crate::style::box_input::BoxStyleInput {
    fn from(style: &BoxStyle) -> Self {
        Self {
            try_source: style.try_style.clone(),
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
