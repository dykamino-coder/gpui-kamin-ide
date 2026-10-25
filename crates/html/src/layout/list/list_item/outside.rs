//! Внешний маркер: логическое начало строки и отступ за рамку главного бокса.

use crate::document::state::ValueCx;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{BoxStyle, Computed, writing_mode::Side};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px, relative};

pub(super) fn content_style(
    marker: Option<BoxStyle>,
    item: &BoxStyle,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    let mut style = marker.unwrap_or_else(|| inherit(item, &Computed::default(), vcx));
    // Анонимный маркер наследует текст, но не размеры главного бокса.
    // Его строчная мера — содержимое, в том числе у вертикального абзаца.
    style.hug_inline = true;
    // Внешний маркер вне потока: фиксированная мера строки пункта
    // не должна растянуть его абзац перед привязкой к inline-start.
    style.orthogonal_inline = None;
    style
}

pub(super) fn position(m: Div, item: &Computed, merged: &Computed) -> Div {
    let wm = merged.wm().used_direction();
    // Начальная позиция нужна коробке без строк; placement совмещает реальные baseline.
    let m = match wm.block_start() {
        Side::Top => m.top_0(),
        Side::Right => m.right_0(),
        Side::Bottom => m.bottom_0(),
        Side::Left => m.left_0(),
    }
    .absolute();
    let b = item.borders();
    let border = [b.top, b.right, b.bottom, b.left][wm.inline_start().index()];
    let border = match border {
        Some(Len::Px(v)) if v > 0.0 => v,
        _ => 0.0,
    };
    // Сохраняем сдвиг за первой строкой, обтекающей предшествующий float.
    let intrusion = merged.flow_shapes.as_ref().map_or(0.0, |shapes| {
        let side = if wm.rtl() { &shapes.1 } else { &shapes.0 };
        side.iter().map(|s| s.cut(0.0, 1.0)).fold(0.0, f32::max)
    });
    // CSS Lists 3 §3.5: outside находится за inline-start границы.
    // Insets отсчитываются от padding box, поэтому его рамка добавляется
    // к отступу конца маркера. Физические left/right годятся лишь для горизонтали.
    let gap = px(border - intrusion);
    let m = match wm.inline_start() {
        Side::Left => m.right(relative(1.)).mr(gap),
        Side::Right => m.left(relative(1.)).ml(gap),
        Side::Top => m.bottom(relative(1.)).mb(gap),
        Side::Bottom => m.top(relative(1.)).mt(gap),
    };
    if wm.rtl() {
        m.text_left()
    } else {
        m.text_right()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Sides, writing_mode::WritingModeKw};

    #[test]
    fn outside_marker_inherits_text_without_principal_box_sizes() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        for kind in [None, Some(crate::style::computed::MarkerType::None)] {
            let mut item = Computed {
                width: Some(Len::Px(120.)),
                height: Some(Len::Px(160.)),
                ..Computed::default()
            };
            item.writing_mode = Some(WritingModeKw::VerticalLr);
            item.font_size = Some(Len::Px(20.));
            item.list_style_type = kind;
            let li = crate::dom::Element::synthetic("li", item, Default::default(), vec![]);
            let mut parent = inherit(&BoxStyle::default(), &li.style.used(), vcx);
            parent.orthogonal_inline = Some(crate::style::computed::orthogonal::InlineConstraint {
                available: 160.,
                fixed: Some(160.),
                min: None,
                max: None,
            });
            let marker = super::super::marker::resolve(&li, &parent, &parent, vcx);
            let style = content_style(marker.and_then(|m| m.style), &parent, vcx);
            assert_eq!(style.width, None);
            assert_eq!(style.height, None);
            assert_eq!(style.font_size, Some(Len::Px(20.)));
            assert_eq!(style.wm(), parent.wm());
            let constraint = crate::layout::writing_mode::native_vertical::constraint(&style, 600.)
                .expect("intrinsic marker inline size");
            assert_eq!(constraint.used(8., 18.), 18.);
        }
    }

    #[test]
    fn outside_marker_uses_inline_start_in_every_writing_mode() {
        for (vertical, rl, sideways, starts, block) in [
            (false, false, false, [Side::Left, Side::Right], Side::Top),
            (true, true, false, [Side::Top, Side::Bottom], Side::Right),
            (true, false, false, [Side::Top, Side::Bottom], Side::Left),
            (true, true, true, [Side::Top, Side::Bottom], Side::Right),
            (true, false, true, [Side::Bottom, Side::Top], Side::Left),
        ] {
            for (rtl, start) in starts.into_iter().enumerate() {
                let mut style = Computed::default();
                style.writing_mode = Some(WritingModeKw::new(vertical, rl, sideways));
                style.rtl = Some(rtl == 1);
                let mut d = position(gpui::div(), &style, &style);
                let e = &d.style().inset;
                let actual = [e.top, e.right, e.bottom, e.left];
                let mut expected = [None; 4];
                expected[block.index()] = Some(px(0.).into());
                expected[start.opposite().index()] = Some(relative(1.).into());
                assert_eq!(actual, expected, "{vertical}/{rl}/{sideways}/{rtl}");
            }
        }
    }

    #[test]
    fn outside_marker_clears_only_the_visible_inline_start_border() {
        for (vertical, sideways, rtl, end, distance) in [
            (false, false, false, Side::Right, 4.),
            (false, false, true, Side::Left, 2.),
            (true, false, false, Side::Bottom, 1.),
            (true, false, true, Side::Top, 3.),
            (true, true, false, Side::Top, 3.),
        ] {
            let mut style = Computed::default();
            style.writing_mode = Some(WritingModeKw::new(vertical, false, sideways));
            style.rtl = Some(rtl);
            style.border_visible = [Some(true); 4];
            style.border_width = Sides {
                top: Some(Len::Px(1.)),
                right: Some(Len::Px(2.)),
                bottom: Some(Len::Px(3.)),
                left: Some(Len::Px(4.)),
            };
            for visible in [true, false] {
                style.border_visible = [Some(visible); 4];
                let mut d = position(gpui::div(), &style, &style);
                let e = &d.style().margin;
                let actual = [e.top, e.right, e.bottom, e.left];
                let mut expected = [None; 4];
                expected[end.index()] = Some(px(if visible { distance } else { 0. }).into());
                assert_eq!(actual, expected);
            }
        }
    }
}
