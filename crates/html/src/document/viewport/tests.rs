//! Regression checks for viewport ownership across html and body.
use super::*;

#[test]
fn viewport_units_use_root_unconditional_gutters_only() {
    for (css, expected) in [
        ("html { overflow: scroll }", [15.0, 15.0]),
        (
            "html { overflow-y: scroll; scrollbar-width: thin }",
            [10.0, 0.0],
        ),
        ("html { overflow: auto }", [0.0, 0.0]),
        (
            "html { overflow: scroll } html::-webkit-scrollbar { width: 300px }",
            [0.0, 0.0],
        ),
        (
            "html { overflow: scroll } body::-webkit-scrollbar { width: 300px }",
            [15.0, 15.0],
        ),
        (
            "html { overflow: scroll } html.unmatched::-webkit-scrollbar { width: 300px }",
            [15.0, 15.0],
        ),
        ("body { overflow: scroll }", [0.0, 0.0]),
        ("html { scrollbar-gutter: stable both-edges }", [30.0, 0.0]),
        (
            "html { overflow: scroll; scrollbar-width: none }",
            [0.0, 0.0],
        ),
    ] {
        let nodes = crate::dom::parse(&format!("<style>{css}</style>"), "");
        let c = policy(&nodes).unwrap();
        assert_eq!(c.viewport_unit_insets, Some(expected), "{css}");
        let mut child = crate::style::cascade::inherit::inherit_unpainted(
            &c,
            &Computed {
                width: Some(Len::Vw(1.0)),
                height: Some(Len::Vh(1.0)),
                ..Computed::default()
            },
        );
        child.resolve_viewport((800.0, 600.0));
        assert_eq!(child.width, Some(Len::Px(800.0 - expected[0])));
        assert_eq!(child.height, Some(Len::Px(600.0 - expected[1])));
        let mut image = crate::layout::table::anon::anon_element("img", vec![]);
        image.style.width = Some(Len::Vw(1.0));
        let mut image = crate::layout::replaced::image::pct_height_to_px(&image, &c);
        image.style.resolve_viewport((800.0, 600.0));
        assert_eq!(image.style.width, Some(Len::Px(800.0 - expected[0])));
    }
}

#[test]
fn body_colors_do_not_propagate_to_the_viewport() {
    let nodes = crate::dom::parse(
        "<style>body { scrollbar-color: red blue; overflow: scroll }</style>",
        "",
    );
    assert!(policy(&nodes).unwrap().scroll.colors.flatten().is_none());
}

#[test]
fn root_width_and_computed_color_survive_body_propagation() {
    let nodes = crate::dom::parse(
        "<style>html { color: red; scrollbar-color: currentColor currentColor; scrollbar-width: none } body { color: blue; scrollbar-width: thin; overflow: scroll }</style>",
        "",
    );
    let c = policy(&nodes).unwrap();
    assert_eq!(
        c.scroll.colors.flatten().unwrap(),
        [crate::style::values::value::Color::parse("red").unwrap(); 2]
    );
    assert_eq!(c.scroll.width, Some(2));
    assert_eq!(c.scroll.current, [false; 2]);
    assert_eq!(c.scroll.forced, [true; 2]);
}
