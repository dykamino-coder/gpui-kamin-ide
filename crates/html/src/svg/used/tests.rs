//! Used SVG sizes follow CSS dimensions and their containing block.

use super::*;
use crate::document::parsed::parse;
use crate::style::values::value::Len;

fn svg() -> Element {
    let nodes = parse(
        r#"<svg width="162" height="81" viewBox="0 0 162 81"></svg>"#,
        "",
    );
    fn find_svg(nodes: &[crate::dom::Node]) -> Option<&Element> {
        for node in nodes {
            if let crate::dom::Node::Element(e) = node {
                if e.tag == "svg" {
                    return Some(e);
                }
                if let Some(svg) = find_svg(&e.children) {
                    return Some(svg);
                }
            }
        }
        None
    }
    find_svg(&nodes).expect("parsed SVG").clone()
}

#[gpui::test]
fn percentage_viewport_uses_css_box_and_auto_ratio(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(800.0), px(600.0)),
        |window, app| {
            let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
            for (width, height, expected) in [
                (Len::Pct(0.2), Len::Auto, (90.0, 45.0)),
                // Avoid an independent percentage rounding step: compare the final size.
                (Len::Pct(1.0), Len::Auto, (450.0, 225.0)),
                (Len::Pct(0.2), Len::Px(30.0), (90.0, 30.0)),
                (Len::Auto, Len::Pct(0.5), (200.0, 100.0)),
            ] {
                let mut e = svg();
                e.style.used_mut().reset_box_layout.width = Some(width);
                e.style.used_mut().reset_box_layout.height = Some(height);
                let mut image = super::super::element(&e, FrameCx::new(&doc)).unwrap();
                let leaf = image.request_layout(window, app);
                let root = window.request_layout(
                    Style {
                        display: gpui::Display::Block,
                        size: size(px(450.0).into(), px(200.0).into()),
                        ..Style::default()
                    },
                    [leaf],
                    app,
                );
                window.compute_layout(
                    root,
                    size(
                        AvailableSpace::Definite(px(450.0)),
                        AvailableSpace::Definite(px(200.0)),
                    ),
                    app,
                );
                assert_eq!(
                    window.layout_size_unrounded(leaf),
                    size(px(expected.0), px(expected.1))
                );
            }
            gpui::div()
        },
    );
}

#[test]
fn presentation_height_is_used_unless_css_auto_overrides_it() {
    let mut e = svg();
    e.style.used_mut().reset_box_layout.width = Some(Len::Pct(0.2));
    assert_eq!(layout_style(&e).0.size.height, px(81.0).into());
    e.style.used_mut().reset_box_layout.height = Some(Len::Auto);
    assert_eq!(layout_style(&e).0.size.height, gpui::Length::Auto);
    assert_eq!(layout_style(&e).0.aspect_ratio, Some(2.0));
}

#[test]
fn natural_dimensions_precede_viewbox_ratio_as_in_chrome() {
    let mut e = svg();
    e.attrs
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case("viewBox"))
        .unwrap()
        .1 = "0 0 100 100".into();
    e.style.used_mut().reset_box_layout.width = Some(Len::Pct(0.2));
    e.style.used_mut().reset_box_layout.height = Some(Len::Auto);
    assert_eq!(layout_style(&e).0.aspect_ratio, Some(2.0));
    e.attrs.retain(|(k, _)| k != "width" && k != "height");
    assert_eq!(layout_style(&e).0.aspect_ratio, Some(1.0));
}

#[test]
fn missing_presentation_height_is_auto() {
    let mut e = svg();
    e.attrs.retain(|(k, _)| k != "height");
    e.style.used_mut().reset_box_layout.width = Some(Len::Pct(0.2));
    assert_eq!(layout_style(&e).0.size.height, gpui::Length::Auto);
    assert_eq!(layout_style(&e).0.aspect_ratio, Some(2.0));
}

#[gpui::test]
fn percentage_viewport_follows_the_current_containing_block(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(800.0), px(600.0)),
        |window, app| {
            let doc = crate::document::state::DocState::new(Default::default());
            let mut e = svg();
            e.style.used_mut().reset_box_layout.width = Some(Len::Pct(0.2));
            e.style.used_mut().reset_box_layout.height = Some(Len::Auto);
            let mut image = super::super::element(&e, FrameCx::new(&doc)).unwrap();
            let leaf = image.request_layout(window, app);
            for width in [450.0, 300.0, 450.0] {
                let root = window.request_layout(
                    Style {
                        display: gpui::Display::Block,
                        size: size(px(width).into(), px(200.0).into()),
                        ..Style::default()
                    },
                    [leaf],
                    app,
                );
                window.compute_layout(
                    root,
                    size(
                        AvailableSpace::Definite(px(width)),
                        AvailableSpace::Definite(px(200.0)),
                    ),
                    app,
                );
                assert_eq!(
                    window.layout_size_unrounded(leaf),
                    size(px(width * 0.2), px(width * 0.1))
                );
            }
            gpui::div()
        },
    );
}
