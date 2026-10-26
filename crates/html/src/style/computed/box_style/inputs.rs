//! Производители входов сборки коробки; нейтральные данные живут в box_facts.

use crate::box_facts::{ApplicabilityInputs, DecorationInputs};

use crate::style::computed::{BoxStyle, Computed, Decor, Display as D, Position as P};

impl BoxStyle {
    pub(crate) fn applicability_inputs(&self) -> ApplicabilityInputs {
        ApplicabilityInputs {
            flex: matches!(self.display, Some(D::Flex | D::InlineFlex)),
            rtl: self.wm().rtl(),
            wrap_reverse: self.flex_wrap_reverse == Some(true),
        }
    }

    pub(crate) fn decoration_inputs(&self, own: &Computed) -> DecorationInputs<'_, Decor> {
        // Сохраняем тест применимости стадии коробки. Преобразование display
        // остаётся в вычисленной стадии (CSS Display 3 §2.7).
        let blockified = matches!(
            self.display,
            Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid | D::GridLanes)
        );
        let blocked = matches!(own.position, Some(P::Absolute | P::Fixed))
            || own.float.is_some_and(|f| f != 0)
            || (!blockified
                && matches!(
                    own.display,
                    Some(D::InlineBlock | D::InlineTable | D::InlineFlex | D::InlineGrid)
                ));
        DecorationInputs {
            inherited: if blocked { &[] } else { &self.decors },
            block: match own.display {
                None => own.block_tag && own.inline_display != Some(true),
                Some(D::Contents) => false,
                Some(_) => true,
            },
            generates_box: own.display != Some(D::Contents),
        }
    }
}
