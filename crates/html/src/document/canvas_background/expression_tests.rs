//! Операнды донора и наследование CSS сохраняют разные контексты цвета.

use super::mark_canvas_background;
use crate::dom::test_support::find;
use crate::style::canvas_background::CanvasBackgroundSource;
use crate::style::cascade::materialize::materialize;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::{Color, Len};
use std::rc::Rc;

#[test]
fn canvas_binds_donor_image_colors_without_rebinding_explicit_inheritance() {
    for image in [
        "linear-gradient(currentColor,currentColor)",
        "linear-gradient(rgb(from currentColor r g b),currentColor)",
        "linear-gradient(currentColor,currentColor),linear-gradient(lime,lime)",
    ] {
        let parsed = crate::dom::parse(
            &format!(
                "<style>html {{ color:red }} head {{ display:block; background-image:inherit }} body {{ color:blue; background-image:{image} }} div {{ color:green; background-image:inherit }}</style><div>x</div>"
            ),
            "",
        );
        let vcx = parsed.values();
        let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
        materialize(&nodes, &Rc::new(Computed::default()), vcx);
        let root = find(&nodes, &|e| e.tag == "html").unwrap();
        let head = find(&nodes, &|e| e.tag == "head").unwrap();
        let body = find(&nodes, &|e| e.tag == "body").unwrap();
        let child = find(&nodes, &|e| e.tag == "div").unwrap();
        let input = root.style.used();
        let canvas = input.canvas_background.clone().unwrap();
        let CanvasBackgroundSource::Donor(background) = &canvas else {
            panic!("expected body donor");
        };
        let blue = Color::parse("blue", vcx).unwrap();
        let green = Color::parse("green", vcx).unwrap();
        assert_eq!(background.gradient.as_ref().unwrap().from, blue);
        assert_eq!(body.style.computed().gradient.as_ref().unwrap().from, blue);
        assert_eq!(
            child.style.computed().gradient.as_ref().unwrap().from,
            green
        );
        assert!(root.style.computed().gradient_raw.is_none());
        assert!(head.style.computed().gradient_raw.is_none());
        assert!(root.style.computed().bg_lists.is_empty());
        assert!(!body.style.used().background_paint.paints());
        assert!(child.style.used().background_paint.paints());
        // Транспорт синтетической коробки сохраняет вход, а не встраивает в CSS.
        let boxed = BoxStyle::shared(input);
        let transported = crate::dom::SharedStyle::layout_made(&boxed);
        let transported_source = transported.used().canvas_background.unwrap();
        let CanvasBackgroundSource::Donor(transported_background) = transported_source else {
            panic!("expected transported body donor");
        };
        assert!(Rc::ptr_eq(&transported_background, background));
        assert!(transported.computed().gradient_raw.is_none());
    }
}

#[test]
fn canvas_keeps_computed_position_expressions_for_single_and_repeated_lists() {
    for image in ["url(a)", "url(a), url(b), url(c)"] {
        let parsed = crate::dom::parse(
            &format!(
                "<style>html {{ font:20px/1 Ahem }} body {{ font-size:40px; background-image:{image}; background-position:min(100%,1em) max(100%,2em), calc(50% - 1em) 0 }}</style>"
            ),
            "",
        );
        let vcx = parsed.values();
        let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
        let root = find(&nodes, &|e| e.tag == "html").unwrap();
        let canvas = root.style.used().canvas_background.unwrap();
        let CanvasBackgroundSource::Donor(background) = &canvas else {
            panic!("expected body donor");
        };
        assert_eq!(
            background.top.position_used((0.0, 0.0), (100.0, 100.0)),
            (40.0, 100.0)
        );
        assert_eq!(
            background.top.position_used((0.0, 0.0), (-50.0, -50.0)),
            (-50.0, 80.0)
        );
        if let Some(layers) = &background.layers {
            assert_eq!(layers.len(), 3);
            assert_eq!(layers[0].position, layers[2].position);
            assert_eq!(
                layers[0].position_used((0.0, 0.0), (-50.0, -50.0)),
                layers[2].position_used((0.0, 0.0), (-50.0, -50.0))
            );
            assert_eq!(layers[1].position.y, Some(Len::Px(0.0)));
            assert_eq!(
                layers[0].position_used((0.0, 0.0), (100.0, 100.0)),
                (40.0, 100.0)
            );
        }
        assert_eq!(root.style.cascaded().bg_pos, Default::default());
    }
}
