//! Анонимная коробка ряда между спаннерами. Декорации принадлежат хосту,
//! ограничения блочного размера — текущему остатку, CSS хоста сохраняется.

use crate::dom::{Element, Node};
use crate::style::values::value::Len;

pub(super) fn segment(host: &Element, children: &[Node], tail_height: Option<f32>) -> Element {
    let mut sub = host.clone();
    sub.children = children.to_vec().into();
    sub.style.used_mut().reset_fragmentation.column_span = None;
    sub.style.used_mut().reset_background.background = None;
    // Всё, что многоколоночник рисует и сдвигает КОРОБКОЙ,
    // уже стоит на хосте `d` (`styled_div_with(e, ..)`);
    // клон ряда повторял это на каждом ряду: контур и
    // тень вокруг каждого ряда, двойная прозрачность,
    // фильтр и трансформ, двойной сдвиг `relative`, а у
    // `position: absolute` ряды выпадали из потока и
    // ложились друг на друга (`multicol-span-all-
    // fieldset-002/003`, `-button-002/003`). Обрезку
    // переполнения делает хост: по css-multicol-1
    // §overflow режет коробка многоколоночника, а не ряд.
    // Ряд остаётся содержащим блоком (`relative`), как
    // прежде.
    sub.style.used_mut().reset_background.gradient = None;
    sub.style.used_mut().reset_background.bg_image = None;
    sub.style.used_mut().reset_border.border_image = None;
    sub.style.used_mut().reset_effects.shadows = Vec::new();
    sub.style.used_mut().reset_effects.inset_shadows = Vec::new();
    sub.style.used_mut().reset_border.outline = None;
    sub.style.used_mut().reset_effects.opacity = None;
    sub.style.used_mut().reset_transforms.transform = None;
    sub.style.used_mut().reset_effects.filter = None;
    sub.style.used_mut().reset_mask.mask_image = None;
    sub.style.used_mut().reset_effects.backdrop_blur = None;
    if matches!(
        sub.style.used().reset_positioned.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        sub.style.used_mut().reset_positioned.position =
            Some(crate::style::computed::Position::Relative);
    }
    sub.style.used_mut().reset_positioned.inset = Default::default();
    sub.style.used_mut().reset_positioned.z_index = None;
    sub.style.used_mut().reset_box_layout.overflow_x = None;
    sub.style.used_mut().reset_box_layout.overflow_y = None;
    sub.style.used_mut().reset_box_layout.margin = Default::default();
    sub.style.used_mut().reset_box_layout.padding = Default::default();
    sub.style.used_mut().reset_border.border_width = Default::default();
    sub.style.used_mut().reset_box_layout.width = None;
    sub.style.used_mut().reset_box_layout.height = tail_height.map(Len::Px);
    // Остаток задаётся один раз: исходная высота хоста не ограничивает ряд.
    sub.marks.spanner_content_box = true;
    sub
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::multicol::dimensions::MulticolDimensions;
    use crate::style::computed::Computed;

    #[test]
    fn row_uses_its_remaining_content_size_without_host_constraints() {
        let host = Element::synthetic(
            "div",
            Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        height: Some(Len::Px(300.)),
                        min_height: Some(Len::Px(200.)),
                        max_height: Some(Len::Px(250.)),
                        min_width: Some(Len::Px(40.)),
                        ..Default::default()
                    },
                ),
                reset_border: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBorder {
                        border_box: Some(true),
                        ..Default::default()
                    },
                ),
                reset_fragmentation: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetFragmentation {
                        column_span: Some(true),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
            Default::default(),
            vec![],
        );
        for remaining in [None, Some(50.)] {
            let row = segment(&host, &[], remaining);
            let dimensions = MulticolDimensions::for_element(&row, &row.style.used());
            assert_eq!(
                dimensions.content_height(&row.style.used(), None, None),
                remaining
            );
            assert_eq!(dimensions.minimum, [Some(Len::Px(40.)), None]);
            assert_eq!(dimensions.maximum[1], None);
            assert!(!dimensions.border_box);
            assert_eq!(row.style.used().reset_fragmentation.column_span, None);
        }
        assert_eq!(
            host.style.used().reset_box_layout.height,
            Some(Len::Px(300.))
        );
        assert_eq!(
            host.style.used().reset_box_layout.min_height,
            Some(Len::Px(200.))
        );
        assert_eq!(
            host.style.used().reset_box_layout.max_height,
            Some(Len::Px(250.))
        );
        assert_eq!(host.style.used().reset_border.border_box, Some(true));
    }
}
