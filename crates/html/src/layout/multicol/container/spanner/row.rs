//! Анонимная коробка ряда между спаннерами. Декорации принадлежат хосту,
//! ограничения блочного размера — текущему остатку, CSS хоста сохраняется.

use crate::dom::{Element, Node};
use crate::style::values::value::Len;

pub(super) fn segment(host: &Element, children: &[Node], tail_height: Option<f32>) -> Element {
    let mut sub = host.clone();
    sub.children = children.to_vec().into();
    sub.style.used_mut().column_span = None;
    sub.style.used_mut().background = None;
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
    sub.style.used_mut().gradient = None;
    sub.style.used_mut().bg_image = None;
    sub.style.used_mut().border_image = None;
    sub.style.used_mut().shadows = Vec::new();
    sub.style.used_mut().inset_shadows = Vec::new();
    sub.style.used_mut().outline = None;
    sub.style.used_mut().opacity = None;
    sub.style.used_mut().transform = None;
    sub.style.used_mut().filter = None;
    sub.style.used_mut().mask_image = None;
    sub.style.used_mut().backdrop_blur = None;
    if matches!(
        sub.style.used().position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        sub.style.used_mut().position = Some(crate::style::computed::Position::Relative);
    }
    sub.style.used_mut().inset = Default::default();
    sub.style.used_mut().z_index = None;
    sub.style.used_mut().overflow_x = None;
    sub.style.used_mut().overflow_y = None;
    sub.style.used_mut().margin = Default::default();
    sub.style.used_mut().padding = Default::default();
    sub.style.used_mut().border_width = Default::default();
    sub.style.used_mut().width = None;
    sub.style.used_mut().min_height = None;
    sub.style.used_mut().max_height = None;
    sub.style.used_mut().border_box = None;
    // Остаток задаётся один раз: исходная высота хоста не ограничивает ряд.
    sub.style.used_mut().height = tail_height.map(Len::Px);
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
                height: Some(Len::Px(300.)),
                min_height: Some(Len::Px(200.)),
                max_height: Some(Len::Px(250.)),
                min_width: Some(Len::Px(40.)),
                border_box: Some(true),
                column_span: Some(true),
                ..Default::default()
            },
            Default::default(),
            vec![],
        );
        for remaining in [None, Some(50.)] {
            let row = segment(&host, &[], remaining);
            let dimensions = MulticolDimensions::css(&row.style.used());
            assert_eq!(
                dimensions.content_height(&row.style.used(), None, None),
                remaining
            );
            assert_eq!(dimensions.minimum, [Some(Len::Px(40.)), None]);
            assert_eq!(dimensions.maximum[1], None);
            assert!(!dimensions.border_box);
            assert_eq!(row.style.used().column_span, None);
        }
        assert_eq!(host.style.used().height, Some(Len::Px(300.)));
        assert_eq!(host.style.used().min_height, Some(Len::Px(200.)));
        assert_eq!(host.style.used().max_height, Some(Len::Px(250.)));
        assert_eq!(host.style.used().border_box, Some(true));
    }
}
