//! Generated filtered images replace elements while retaining their box style.

use super::{first_element, parse};
use crate::style::values::value::Len;

#[test]
fn filtered_content_replaces_children_and_preserves_size() {
    let src = "filter(url(data:image/png;base64,iVBORw0KGgo=), invert(1))";
    let nodes = parse(
        &format!("<div style='width:100px;height:50px;content:{src}'><span>hidden</span></div>"),
        "",
    );
    let element = first_element(&nodes);
    assert_eq!(element.tag, "img");
    assert_eq!(element.attr("src"), Some(src));
    assert!(element.children.is_empty());
    assert_eq!(element.style.width, Some(Len::Px(100.0)));
    assert_eq!(element.style.height, Some(Len::Px(50.0)));
}

#[test]
fn missing_filtered_content_does_not_replace_element() {
    let nodes = parse(
        "<div style='content:filter(url(missing-filtered-image.png), invert(1))'>visible</div>",
        "",
    );
    let element = first_element(&nodes);
    assert_eq!(element.tag, "div");
    assert!(!element.children.is_empty());
}
