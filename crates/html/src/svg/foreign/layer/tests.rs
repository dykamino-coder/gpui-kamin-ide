//! The embedded viewport root of `foreignObject`: a typed layout box that
//! keeps the values the SVG host hands down and drops the host's effects.

use super::*;
use crate::style::box_input::BoxOrigin;
use crate::style::computed::Computed;

#[test]
fn embedded_root_is_a_clipped_block_without_host_effects() {
    let mut host = Computed {
        opacity: Some(0.5),
        position: Some(Position::Absolute),
        display: Some(Display::InlineBlock),
        overflow_y: Some(Overflow::Scroll),
        ..Computed::default()
    };
    host.font_size = Some(Len::Px(20.0));
    let host = BoxStyle::new(host);
    let root = embedded_root(host.clone(), (30.0, 20.0));
    assert_eq!(root.width, Some(Len::Px(30.0)));
    assert_eq!(root.height, Some(Len::Px(20.0)));
    assert_eq!(root.display, Some(Display::Block));
    assert_eq!(root.position, Some(Position::Relative));
    assert_eq!(root.flow_root, Some(true));
    assert_eq!(root.opacity, None);
    assert_eq!(root.overflow_x, Some(Overflow::Hidden));
    // An authored overflow stays; inherited values reach the content.
    assert_eq!(root.overflow_y, Some(Overflow::Scroll));
    assert_eq!(root.font_size, Some(Len::Px(20.0)));
    // The host box itself is not edited.
    assert_eq!(host.opacity, Some(0.5));
    assert_eq!(host.display, Some(Display::InlineBlock));
}

#[test]
fn embedded_root_reenters_under_the_default_viewport_parent() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    let mut host = Computed::default();
    host.font_size = Some(Len::Px(20.0));
    let host = BoxStyle::new(host);
    let mut html = crate::dom::table_boxes::anon_element("div", vec![]);
    html.set_layout_box(LayoutBoxInput::new(
        BoxOrigin::EmbeddedRoot,
        embedded_root(host, (30.0, 20.0)),
    ));
    assert_eq!(html.marks.box_origin, Some(BoxOrigin::EmbeddedRoot));
    let boxed = box_style(&BoxStyle::default(), &html.style, vcx);
    assert_eq!(boxed.font_size, Some(Len::Px(20.0)));
    assert_eq!(boxed.width, Some(Len::Px(30.0)));
    assert_eq!(boxed.display, Some(Display::Block));
}
