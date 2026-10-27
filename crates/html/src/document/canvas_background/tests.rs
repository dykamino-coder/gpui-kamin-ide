//! Донор фона холста сохраняет свой вычисленный цвет при распространении.

use super::*;
use crate::dom::test_support::find;
use crate::style::values::value::{Color, StyleColor};
use std::rc::Rc;

#[test]
fn body_currentcolor_uses_its_computed_color_with_or_without_an_image() {
    for image in ["", "background-image:url(clear.png)"] {
        for color in ["blue", "inherit", "currentColor"] {
            let root_color = if color == "blue" { "red" } else { "blue" };
            let parsed = crate::dom::parse(
                &format!(
                    "<style>html {{ color:{root_color} }} body {{ color:{color}; background-color:currentColor; {image} }}</style>"
                ),
                "",
            );
            let vcx = parsed.values();
            let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
            crate::style::cascade::materialize::materialize(
                &nodes,
                &Rc::new(Computed::default()),
                vcx,
            );
            let root = find(&nodes, &|e| e.tag == "html").unwrap();
            let body = find(&nodes, &|e| e.tag == "body").unwrap();
            let blue = Color::parse("blue", vcx).unwrap();
            assert!(root.style.computed().canvas_bg);
            assert_eq!(
                root.style.used().canvas_background.as_ref().unwrap().color,
                Some(blue),
                "{color}; {image}"
            );
            assert_eq!(root.style.computed().color, Color::parse(root_color, vcx));
            assert_eq!(body.style.computed().color, Some(blue));
            assert_eq!(body.style.computed().background, Some(blue));
            assert!(!body.style.used().background_paint.paints());
        }
    }
}

#[test]
fn root_currentcolor_selects_the_root_and_leaves_body_background_in_place() {
    let parsed = crate::dom::parse(
        "<style>html { color:red; background-color:currentColor } body { color:blue; background:green }</style>",
        "",
    );
    let vcx = parsed.values();
    let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
    let root = find(&nodes, &|e| e.tag == "html").unwrap();
    let body = find(&nodes, &|e| e.tag == "body").unwrap();
    assert!(root.style.cascaded().canvas_bg);
    assert_eq!(
        root.style.cascaded().background_color,
        Some(StyleColor::Current)
    );
    assert_eq!(body.style.cascaded().background, Color::parse("green", vcx));
}

#[test]
fn transparent_root_currentcolor_still_allows_body_propagation() {
    let parsed = crate::dom::parse(
        "<style>html { color:transparent; background-color:currentColor } body { color:blue; background-color:currentColor; filter:invert(1) }</style>",
        "",
    );
    let vcx = parsed.values();
    let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
    let root = find(&nodes, &|e| e.tag == "html").unwrap();
    assert!(root.style.cascaded().canvas_bg);
    assert_eq!(
        root.style.used().canvas_background.as_ref().unwrap().color,
        Color::parse("blue", vcx)
    );
}
