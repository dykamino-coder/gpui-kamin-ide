//! Regression checks for the SVG reference box and CTM assembly.

use super::element_transform;
use crate::dom::{Element, Node, parse};
use crate::style::values::value::Len;

fn rect(markup: &str) -> Element {
    fn find(nodes: &[Node]) -> Option<Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.tag == "rect" => Some(e.clone()),
            Node::Element(e) => find(&e.children),
            Node::Text(_) => None,
        })
    }
    find(&parse(markup, "")).expect("rect")
}

#[test]
fn default_percentages_use_viewport_not_shape_bounds() {
    let mut e = rect(r#"<svg><rect x="25" y="40" width="50" height="60"/></svg>"#);
    e.style.transform = Some(Default::default());
    e.style.transform_origin = Some((0.25, 0.5));
    e.style.translate = Some((Len::Pct(0.5), Len::Pct(0.25)));
    crate::svg::VIEW_BOX.with(|v| {
        let old = v.replace((200.0, 300.0));
        let result = element_transform(&e).0;
        v.set(old);
        assert_eq!(
            result.as_deref(),
            Some("translate(50 150) translate(100 75)  translate(-50 -150)")
        );
    });
}

#[test]
fn fill_box_lengths_start_at_object_bounds() {
    let e = rect(
        r#"<svg><rect x="25" y="40" width="50" height="60" style="transform-box:fill-box" transform-origin="10px 20px" transform="scale(2)"/></svg>"#,
    );
    assert_eq!(
        element_transform(&e).0.as_deref(),
        Some("translate(35 60) scale(2) translate(-35 -60)")
    );
}

#[test]
fn individual_properties_precede_the_presentation_transform() {
    let mut e = rect(r#"<svg><rect transform="translate(10 20)"/></svg>"#);
    e.style.transform = Some(Default::default());
    e.style.translate = Some((Len::Px(3.0), Len::Px(4.0)));
    e.style.rotate_prop = Some(std::f32::consts::FRAC_PI_2);
    e.style.scale_prop = Some((2.0, 3.0));
    assert_eq!(
        element_transform(&e).0.as_deref(),
        Some("translate(3 4) rotate(90) scale(2 3) translate(10 20)")
    );
}
