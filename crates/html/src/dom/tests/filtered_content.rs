//! Тесты `content: filter(url(…), …)` на элементе: загруженная картинка
//! замещает элемент (`img` с `src`, без детей) и сохраняет его размеры;
//! ненайденная картинка замены не делает.

use super::{first_element, parse};
use crate::style::computed::ImageSource;
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
    let url = "data:image/png;base64,iVBORw0KGgo=";
    assert_eq!(element.attr("src"), Some(url));
    let Some(ImageSource::Filter { image, program }) = element.style.content_image() else {
        panic!("generated image retains its computed filter");
    };
    assert_eq!(image.as_ref(), &ImageSource::Url(url.to_string()));
    assert_eq!(program.filter.unwrap().invert, 1.0);
    assert_eq!(program.calls, 1);
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
