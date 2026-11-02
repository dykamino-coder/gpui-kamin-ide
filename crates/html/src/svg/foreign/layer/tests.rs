//! The embedded viewport root of `foreignObject`: a typed layout box that
//! keeps the values the SVG host hands down and drops the host's effects.

use super::*;
use crate::style::box_input::BoxOrigin;
use crate::style::computed::Computed;

#[test]
fn embedded_root_is_a_clipped_block_without_host_effects() {
    let mut host = Computed {
        reset_effects: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetEffects {
                opacity: Some(0.5),
                ..Default::default()
            },
        ),
        reset_positioned: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetPositioned {
                position: Some(Position::Absolute),
                ..Default::default()
            },
        ),
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                display: Some(Display::InlineBlock),
                overflow_y: Some(Overflow::Scroll),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    host.inh.font.font_size = Some(Len::Px(20.0));
    let host = BoxStyle::new(host);
    let root = embedded_root(host.clone(), (30.0, 20.0));
    assert_eq!(root.reset_box_layout.width, Some(Len::Px(30.0)));
    assert_eq!(root.reset_box_layout.height, Some(Len::Px(20.0)));
    assert_eq!(root.reset_box_layout.display, Some(Display::Block));
    assert_eq!(root.reset_positioned.position, Some(Position::Relative));
    assert_eq!(root.reset_context.flow_root, Some(true));
    assert_eq!(root.reset_effects.opacity, None);
    assert_eq!(root.reset_box_layout.overflow_x, Some(Overflow::Hidden));
    // An authored overflow stays; inherited values reach the content.
    assert_eq!(root.reset_box_layout.overflow_y, Some(Overflow::Scroll));
    assert_eq!(root.inh.font.font_size, Some(Len::Px(20.0)));
    // The host box itself is not edited.
    assert_eq!(host.reset_effects.opacity, Some(0.5));
    assert_eq!(host.reset_box_layout.display, Some(Display::InlineBlock));
}

#[test]
fn embedded_root_reenters_under_the_default_viewport_parent() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    let mut host = Computed::default();
    host.inh.font.font_size = Some(Len::Px(20.0));
    let host = BoxStyle::new(host);
    let mut html = crate::dom::table_boxes::anon_element("div", vec![]);
    html.set_layout_box(LayoutBoxInput::new(
        BoxOrigin::EmbeddedRoot,
        embedded_root(host, (30.0, 20.0)),
    ));
    assert_eq!(html.marks.box_origin, Some(BoxOrigin::EmbeddedRoot));
    let boxed = box_style(&BoxStyle::default(), &html.style, vcx);
    assert_eq!(boxed.inh.font.font_size, Some(Len::Px(20.0)));
    assert_eq!(boxed.reset_box_layout.width, Some(Len::Px(30.0)));
    assert_eq!(boxed.reset_box_layout.display, Some(Display::Block));
}
