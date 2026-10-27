//! Роль холста принадлежит коробке и сохраняется независимо от CSS.

use super::mark_canvas_background;
use crate::dom::{SharedStyle, test_support::find};
use crate::style::canvas_background::CanvasBackgroundSource;
use crate::style::cascade::materialize::{box_style, materialize};
use crate::style::computed::{BoxStyle, Computed};
use std::rc::Rc;

#[test]
fn own_root_source_survives_box_transport_without_rewriting_or_inheriting_css() {
    for display in ["block", "contents"] {
        let parsed = crate::dom::parse(
            &format!(
                "<style>html {{ display:{display}; color:red; background:currentColor; border-radius:23px }} body {{ color:blue; background:green }} div {{ background:inherit }}</style><div>x</div>"
            ),
            "",
        );
        let vcx = parsed.values();
        let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
        materialize(&nodes, &Rc::new(Computed::default()), vcx);
        let root = find(&nodes, &|e| e.tag == "html").unwrap();
        let body = find(&nodes, &|e| e.tag == "body").unwrap();
        let child = find(&nodes, &|e| e.tag == "div").unwrap();
        let boxed = box_style(&BoxStyle::default(), &root.style, vcx);
        let source = boxed.canvas_background.clone().unwrap();
        assert_eq!(source, CanvasBackgroundSource::Root);
        assert_eq!(boxed.background, root.style.computed().background);
        assert_eq!(boxed.radius, root.style.computed().radius);
        let body_box = box_style(&boxed, &body.style, vcx);
        let child_box = box_style(&body_box, &child.style, vcx);
        assert!(body_box.canvas_background.is_none());
        assert!(child_box.canvas_background.is_none());
        assert!(body_box.background_paint.paints());
        assert_eq!(
            child.style.computed().background,
            body.style.computed().background
        );
        // Заимствованный, потребляющий и restyle-пути сохраняют ту же роль.
        let restyled = boxed.restyled((**boxed.style_rc()).clone());
        for transported in [
            SharedStyle::layout_made(&boxed),
            SharedStyle::layout_made(BoxStyle::shared(&boxed)),
            SharedStyle::layout_made(BoxStyle::shared(restyled)),
        ] {
            let input = transported.used();
            assert_eq!(input.canvas_background.as_ref(), Some(&source));
            assert_eq!(input.background, boxed.background);
            assert_eq!(input.radius, boxed.radius);
        }
    }
}

#[test]
fn canvas_role_selection_respects_containment_and_box_generation() {
    for (root_css, body_css, expected) in [
        ("background:red", "background:blue", Some(true)),
        ("background:transparent", "background:blue", Some(false)),
        ("background:red;contain:style", "background:blue", None),
        ("background:red;display:none", "", None),
        ("", "background:blue;contain:layout", None),
        ("", "background:blue;display:contents", None),
        ("", "", None),
    ] {
        let parsed = crate::dom::parse(
            &format!("<style>html {{{root_css}}} body {{{body_css}}}</style><body>x</body>"),
            "",
        );
        let nodes = mark_canvas_background(parsed.nodes.clone(), parsed.values());
        let root = find(&nodes, &|e| e.tag == "html").unwrap();
        let role = root.style.used().canvas_background;
        assert_eq!(
            role.as_ref()
                .map(|s| matches!(s, CanvasBackgroundSource::Root)),
            expected,
            "{root_css}; {body_css}"
        );
        if let Some(body) = find(&nodes, &|e| e.tag == "body") {
            assert!(body.style.used().canvas_background.is_none());
            assert_eq!(
                body.style.used().background_paint.paints(),
                expected != Some(false)
            );
        } else {
            assert!(root_css.contains("display:none"));
        }
    }
}
