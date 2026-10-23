//! Использованные края и процентные размеры каркаса из исходного CB.

use crate::document::state::ValueCx;
use crate::style::values::value::{Len, calc_get};

pub(super) fn resolved_style(
    c: &crate::style::computed::BoxStyle,
    used: gpui::MeasuredBox,
    vcx: ValueCx<'_>,
) -> crate::style::computed::BoxStyle {
    let mut frame = c.clone();
    let resolve = |value: Option<Len>, basis: Option<gpui::Pixels>| {
        value.and_then(|v| match v {
            Len::Pct(_) => v
                .to_px(basis.map(f32::from), c.font_cx(vcx), vcx)
                .map(Len::Px),
            Len::Calc(id) if calc_get(id, vcx).pct_px().is_some() => v
                .to_px(basis.map(f32::from), c.font_cx(vcx), vcx)
                .map(Len::Px),
            _ => Some(v),
        })
    };
    frame.padding.top = Some(Len::Px(used.padding.top.into()));
    frame.padding.right = Some(Len::Px(used.padding.right.into()));
    frame.padding.bottom = Some(Len::Px(used.padding.bottom.into()));
    frame.padding.left = Some(Len::Px(used.padding.left.into()));
    let style: &mut crate::style::computed::Computed = &mut frame;
    for (axis, basis) in [used.parent_size.width, used.parent_size.height]
        .into_iter()
        .enumerate()
    {
        let frame = &mut *style;
        if !c.stretch_size[axis] {
            let dimension = if axis == 0 {
                &mut frame.width
            } else {
                &mut frame.height
            };
            *dimension = resolve(*dimension, basis);
        }
        let (min, max) = if axis == 0 {
            (&mut frame.min_width, &mut frame.max_width)
        } else {
            (&mut frame.min_height, &mut frame.max_height)
        };
        *min = resolve(*min, basis);
        *max = resolve(*max, basis);
        frame.fit_arg[axis] = resolve(frame.fit_arg[axis], basis);
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;
    use gpui::{px, size};

    #[test]
    fn padding_and_percentage_sizes_keep_original_cb_in_both_builds() {
        let doc = crate::document::state::DocState::new(Default::default());
        let c = Computed {
            width: Some(Len::Pct(0.5)),
            height: Some(Len::Pct(0.5)),
            min_height: Some(Len::Pct(0.25)),
            ..Default::default()
        };
        let used = gpui::MeasuredBox {
            parent_size: size(Some(px(300.)), Some(px(200.))),
            padding: gpui::Edges {
                top: px(30.),
                right: px(30.),
                bottom: px(30.),
                left: px(30.),
            },
            ..Default::default()
        };
        let values = doc.frame_values();
        for border_box in [false, true] {
            let c = Computed {
                border_box: Some(border_box),
                ..c.clone()
            };
            let frame = resolved_style(
                &crate::style::computed::BoxStyle::from(c.clone()),
                used,
                values,
            );
            assert_eq!(frame.width, Some(Len::Px(150.)));
            assert_eq!(frame.height, Some(Len::Px(100.)));
            assert_eq!(frame.min_height, Some(Len::Px(50.)));
            assert_eq!(frame.padding.top, Some(Len::Px(30.)));
            use gpui::Styled;
            let mut element = crate::paint::apply::apply(gpui::div(), &frame, values);
            super::super::frame(&mut element, Some(px(210.)));
            assert_eq!(
                element.style().size.height,
                Some(px(if border_box { 100. } else { 160. }).into())
            );
            assert_eq!(
                element.style().min_size.height,
                Some(px(if border_box { 50. } else { 110. }).into())
            );
        }
    }
}
