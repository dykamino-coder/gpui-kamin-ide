//! Собственный used-фон донора не меняет наследование и оформление его потомков.

use super::mark_canvas_background;
use crate::dom::test_support::find;
use crate::style::background_paint::*;
use crate::style::canvas_background::CanvasBackgroundSource;
use crate::style::cascade::materialize::{box_style, materialize};
use crate::style::computed::{BoxStyle, Computed};
use std::rc::Rc;

#[test]
fn donor_computed_background_remains_the_source_for_explicit_inheritance() {
    let parsed = crate::dom::parse(
        "<style>body { color:blue; background:currentColor linear-gradient(blue,blue) 12px 18px / 100px 120px no-repeat } div { background:inherit }</style><div>x</div>",
        "",
    );
    let vcx = parsed.values();
    let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
    materialize(&nodes, &Rc::new(Computed::default()), vcx);
    let root = find(&nodes, &|e| e.tag == "html").unwrap();
    let body = find(&nodes, &|e| e.tag == "body").unwrap();
    let child = find(&nodes, &|e| e.tag == "div").unwrap();
    let donor = body.style.computed();
    let inherited = child.style.computed();
    let canvas = root.style.used().canvas_background.unwrap();
    let CanvasBackgroundSource::Donor(background) = &canvas else {
        panic!("expected body donor");
    };
    assert!(root.style.used().canvas_background.is_some());
    assert_eq!(inherited.background, donor.background);
    assert_eq!(inherited.gradient_raw, donor.gradient_raw);
    assert_eq!(inherited.bg_size, donor.bg_size);
    assert_eq!(inherited.bg_pos, donor.bg_pos);
    assert_eq!(inherited.bg_repeat, donor.bg_repeat);
    assert_eq!(background.color, donor.background);
    assert_eq!(background.gradient_image, donor.gradient_raw);
    assert_eq!(background.top.size, donor.bg_size);
    assert_eq!(background.top.position, donor.bg_pos);
    assert_eq!(background.top.repeat, donor.bg_repeat);
    assert!(root.style.computed().gradient_raw.is_none());
    assert!(donor.gradient.is_some());
    let root_box = box_style(&BoxStyle::default(), &root.style, vcx);
    let body_box = box_style(&root_box, &body.style, vcx);
    let child_box = box_style(&body_box, &child.style, vcx);
    assert_eq!(body_box.background_paint, BackgroundPaint::Initial);
    assert_eq!(child_box.background_paint, BackgroundPaint::Css);
    // Повторный вход синтетической коробки сохраняет собственное оформление.
    let transported = crate::dom::SharedStyle::layout_made(&body_box);
    assert_eq!(
        transported.used().background_paint,
        BackgroundPaint::Initial
    );
    assert!(Rc::ptr_eq(&transported.used(), body_box.style_rc()));
}

#[test]
fn initial_used_background_suppresses_paint_without_erasing_css_or_borders() {
    use gpui::Styled;
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for background in [
        "blue",
        "blue padding-box",
        "linear-gradient(blue,blue)",
        "linear-gradient(rgba(0,0,255,.5),rgba(0,0,255,.5)) 0 0 / 100px 100px no-repeat",
        "linear-gradient(rgba(0,0,255,.5),rgba(0,0,255,.5)),linear-gradient(transparent,transparent)",
        "url(a)",
        "blue border-area",
    ] {
        let mut css = Computed::default();
        css.apply_one("background", background, vcx);
        let mut background_only = BoxStyle::new(css.clone());
        background_only.background_paint = BackgroundPaint::Initial;
        // Начальный фон исключает также плитки из сохранённых CSS-списков.
        assert!(
            crate::paint::decorations::decorations(&background_only, true, vcx, None).is_empty()
        );
        css.apply_one("border", "10px solid red", vcx);
        let mut body = BoxStyle::new(css);
        let source = body.style_rc().clone();
        body.background_paint = BackgroundPaint::Initial;
        let mut painted = crate::paint::apply::apply_paint(gpui::div(), &body, vcx);
        assert!(painted.style().background.is_none(), "{background}");
        assert!(crate::paint::background::color_layer(&body, vcx).is_none());
        assert_eq!(
            crate::paint::background::border_paint(&body, body.used_border_color(0), vcx),
            body.used_border_color(0)
        );
        assert!(Rc::ptr_eq(&source, body.style_rc()));
        // После подавления body тот же CSS нормально красит независимую коробку.
        let descendant = BoxStyle::shared(source);
        assert_eq!(descendant.background_paint, BackgroundPaint::Css);
        if background == "blue" {
            painted = crate::paint::apply::apply_paint(gpui::div(), &descendant, vcx);
            assert!(painted.style().background.is_some());
        }
    }
}

#[test]
fn suppressed_background_does_not_supply_a_drop_shadow_silhouette() {
    let parsed = crate::dom::parse(
        "<style>body { background:blue; filter:drop-shadow(4px 4px 2px red) } div { background:inherit }</style><div>x</div>",
        "",
    );
    let vcx = parsed.values();
    let nodes = mark_canvas_background(parsed.nodes.clone(), vcx);
    materialize(&nodes, &Rc::new(Computed::default()), vcx);
    let root = find(&nodes, &|e| e.tag == "html").unwrap();
    let body = find(&nodes, &|e| e.tag == "body").unwrap();
    let child = find(&nodes, &|e| e.tag == "div").unwrap();
    let root_box = box_style(&BoxStyle::default(), &root.style, vcx);
    let body_box = box_style(&root_box, &body.style, vcx);
    let child_box = box_style(&body_box, &child.style, vcx);
    assert!(body_box.drop_shadow.is_some());
    assert!(body_box.paint_shadows().is_empty());
    assert_eq!(body_box.shadows, body.style.computed().shadows);
    let ordinary = box_style(
        &root_box,
        &crate::dom::SharedStyle::from(body.style.cascaded().clone()),
        vcx,
    );
    assert_eq!(ordinary.paint_shadows().len(), 1);
    assert_eq!(ordinary.shadows, body.style.computed().shadows);
    assert_eq!(child_box.background_paint, BackgroundPaint::Css);
    assert_eq!(body_box.foreground_paint().fill, None);
    assert_eq!(
        body.style.computed().background,
        child.style.computed().background
    );
}

#[test]
fn initial_background_does_not_supply_text_clip_fill_or_change_corner_geometry() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut css = Computed::default();
    css.apply_one("color", "transparent", vcx);
    css.apply_one("background", "blue text", vcx);
    let mut input = crate::dom::SharedStyle::from(css);
    let parent = BoxStyle::default();
    let ordinary = box_style(&parent, &input, vcx);
    assert!(ordinary.foreground_paint().fill.is_some());
    input.set_background_paint(BackgroundPaint::Initial);
    let suppressed = box_style(&parent, &input, vcx);
    assert_eq!(suppressed.foreground_paint().fill, None);
    assert_eq!(suppressed.color, input.computed().color);

    let mut css = Computed::default();
    css.apply_one("background", "url(a)", vcx);
    css.apply_one("border-radius", "100px 0 0 0", vcx);
    css.apply_one("width", "100px", vcx);
    css.apply_one("height", "100px", vcx);
    let mut body = BoxStyle::new(css);
    let source = body.style_rc().clone();
    assert!(!crate::paint::background::single_corner::eligible(&body));
    body.background_paint = BackgroundPaint::Initial;
    assert!(crate::paint::background::single_corner::eligible(&body));
    assert_eq!(body.radius, source.radius);
    assert!(Rc::ptr_eq(&source, body.style_rc()));
}
