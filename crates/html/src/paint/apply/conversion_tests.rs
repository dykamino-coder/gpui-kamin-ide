use super::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Color, Len};
use std::rc::Rc;

fn styles() -> Vec<BoxStyle> {
    let mut sized = Computed::default();
    sized.reset_box_layout.width = Some(Len::Px(40.0));
    sized.reset_box_layout.height = Some(Len::Pct(0.5));
    let mut painted = sized.clone();
    painted.reset_background.background = Some(Color {
        r: 0.1,
        g: 0.8,
        b: 0.2,
        a: 1.0,
    });
    painted.reset_box_layout.display = Some(Display::InlineBlock);
    let mut text = Computed::default();
    text.inh.font.font_weight = Some(700);
    text.inh.font.italic = Some(true);
    [Computed::default(), sized, painted, text]
        .into_iter()
        .map(BoxStyle::new)
        .collect()
}

#[test]
fn a_reused_conversion_equals_a_fresh_one_and_is_shared() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.frame_values().with_viewport((800.0, 600.0));
    let memo = RefCell::new(StyleConversions::default());
    for style in styles() {
        let dimensions = BoxDimensions::from_computed(&style);
        let mut first = converted_div(&style, dimensions, vcx, &memo);
        let mut twin = BoxStyle::new((*style).clone());
        twin.self_node = 77;
        let mut second = converted_div(&twin, dimensions, vcx, &memo);
        let mut fresh = apply_for_decorations_with_dimensions(div(), &style, Some(dimensions), vcx);
        assert_eq!(
            *second.interactivity().base_style,
            *fresh.interactivity().base_style
        );
        assert!(Rc::ptr_eq(
            &first.interactivity().base_style,
            &second.interactivity().base_style
        ));
    }
}
