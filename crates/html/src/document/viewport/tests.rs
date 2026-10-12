//! Regression checks for viewport ownership across html and body.
use super::*;

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
