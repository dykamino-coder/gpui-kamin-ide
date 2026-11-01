//! Гейт многострочного flex-ряда для сбора краёв (`row_container`) и его главный размер
//! (`main_size`): из кадра меры строк (`ValueCx::lines`) или из ширины в точках.

use crate::dom::Element;
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::cx::FrameCx;
use crate::style::computed::{Align, Display, FlexDir, Justify};
use crate::style::values::value::{Len, px_auto_zero};

pub(crate) fn row_container(c: &Element) -> bool {
    let s = &c.style.used();
    s.reset_box_layout.display == Some(Display::Flex)
        && s.reset_flex.webkit_box != Some(true)
        && matches!(s.reset_flex.flex_dir, None | Some(FlexDir::Row))
        && s.reset_flex.flex_wrap == Some(true)
        && s.reset_flex.flex_wrap_reverse != Some(true)
        && matches!(s.reset_box_layout.height, None | Some(Len::Auto))
        && s.reset_flex.flex_balance != Some(true)
        && !s.wm().vertical()
        && !s.wm().rtl()
        && matches!(
            s.reset_flex.align_items,
            None | Some(Align::Stretch) | Some(Align::Start)
        )
        && matches!(
            s.reset_flex.align_content,
            None | Some(Justify::Start) | Some(Justify::Stretch)
        )
}

pub(crate) fn main_size(c: &Element, vcx: FrameCx<'_>) -> Option<f32> {
    vcx.lines
        .and_then(|cx| cx.frames.last())
        .and_then(|f| f.w)
        .or_else(|| {
            let Some(Len::Px(w)) = c.style.used().reset_box_layout.width else {
                return None;
            };
            let b = c.style.used().borders();
            let edges = px_auto_zero(c.style.used().reset_box_layout.padding.left)?
                + px_auto_zero(c.style.used().reset_box_layout.padding.right)?
                + px_auto_zero(b.left)?
                + px_auto_zero(b.right)?;
            Some(
                if BoxDimensions::for_element(c, &c.style.used()).border_box {
                    (w - edges).max(0.0)
                } else {
                    w
                },
            )
        })
}
