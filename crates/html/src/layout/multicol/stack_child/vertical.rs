//! Фиксация строчной оси измеряемой вертикальной копии: колонка — ряд высотой в строчный
//! размер колонки, копия стоит у начала строчной оси содержащего блока.

use crate::dom::Element;
use crate::style::computed::BoxStyle;
use gpui::{IntoElement, ParentElement, Styled};

pub(super) fn wrap(
    element: gpui::AnyElement,
    copy: &Element,
    container: &BoxStyle,
    vertical: bool,
    reverse: bool,
    inline: Option<f32>,
) -> gpui::AnyElement {
    let Some(inline) =
        inline.filter(|_| vertical && crate::layout::fragment::flow::vertical_copy(copy, reverse))
    else {
        return element;
    };
    let wrapper = gpui::div().flex().h(gpui::px(inline));
    let wrapper = if reverse {
        wrapper.flex_row_reverse()
    } else {
        wrapper.flex_row()
    };
    // CSS 2.1 §10.3.3 in logical terms (css-writing-modes-4 §7.3): an
    // over-constrained block sits at the inline-start of its containing block.
    // The inline axis runs bottom-to-top for `sideways-lr` with `ltr` and
    // for the other vertical modes with `rtl` (§2.1, §3.1).
    let wrapper = if inline_start_at_bottom(container) {
        wrapper.items_end()
    } else {
        wrapper
    };
    wrapper.child(element).into_any_element()
}

/// Whether the containing block's inline axis starts at the physical bottom.
fn inline_start_at_bottom(container: &BoxStyle) -> bool {
    let wm = container.wm();
    wm.sideways_lr() != wm.rtl()
}

#[cfg(test)]
mod tests {
    use super::inline_start_at_bottom;
    use crate::style::computed::{BoxStyle, Computed};

    #[test]
    fn inline_start_is_at_the_bottom_for_sideways_lr_and_rtl_vertical_flows() {
        let doc = crate::document::state::DocState::new(Default::default());
        for (css, bottom) in [
            ("writing-mode:vertical-rl", false),
            ("writing-mode:vertical-lr", false),
            ("writing-mode:sideways-rl", false),
            ("writing-mode:sideways-lr", true),
            ("writing-mode:vertical-rl;direction:rtl", true),
            ("writing-mode:sideways-lr;direction:rtl", false),
        ] {
            let mut style = Computed::default();
            style.apply_decls(&crate::style::css::parse_decls(css), doc.values());
            let merged =
                crate::style::cascade::inherit::inherit(&BoxStyle::default(), &style, doc.values());
            assert_eq!(inline_start_at_bottom(&merged), bottom, "{css}");
        }
    }
}
