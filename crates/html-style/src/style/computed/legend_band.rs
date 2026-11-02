//! Стиль анонимной полосы отрисованной легенды `fieldset` (HTML §15.3.13).

use super::{Align, Computed, Display, Justify, Sides};
use crate::style::values::value::Len;

/// Гибкий ряд высотой не меньше начальной рамки `border`, легенда в нём по
/// центру блочной оси; поднят через рамку и отбивку `pad` к краю border box и
/// отпускает содержимое на `pad` ниже себя. `justify` — строчное выравнивание
/// легенды.
pub fn legend_band(border: f32, pad: f32, justify: Option<Justify>) -> Computed {
    Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                display: Some(Display::Flex),
                min_height: Some(Len::Px(border)),
                margin: Sides {
                    top: Some(Len::Px(-(border + pad))),
                    bottom: Some(Len::Px(pad)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        reset_flex: crate::style::computed::StyleGroup::new(crate::style::computed::ResetFlex {
            align_items: Some(Align::Center),
            justify_content: justify,
            ..Default::default()
        }),
        ..Default::default()
    }
}
