//! Краска вычисленного цвета элемента: фильтр и заливка текста применяются один раз.

use super::tests::element;
use super::*;

#[test]
fn inherited_element_color_is_filtered_once_per_box() {
    let parsed = crate::dom::parse(
        r#"<body><div style="color:red;filter:brightness(50%)"><span><i>x</i></span></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let mut parent = BoxStyle::default();
    for tag in ["div", "span", "i"] {
        let node = element(&parsed.nodes, tag);
        assert_eq!(node.style.computed().color.unwrap().r, 1.0);
        parent = box_style(&parent, &node.style, vcx);
        assert_eq!(parent.color.unwrap().r, 0.5);
    }
}

#[test]
fn text_clip_fill_does_not_change_element_inheritance() {
    let parsed = crate::dom::parse(
        r#"<body><div style="color:transparent;background:red;background-clip:text"><span><i>x</i></span><b style="position:absolute">y</b></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = box_style(
        &BoxStyle::default(),
        &element(&parsed.nodes, "div").style,
        vcx,
    );
    let span = box_style(&div, &element(&parsed.nodes, "span").style, vcx);
    let i = box_style(&span, &element(&parsed.nodes, "i").style, vcx);
    let b = box_style(&div, &element(&parsed.nodes, "b").style, vcx);
    assert_eq!(div.color, span.color);
    assert_eq!(span.color, i.color);
    assert_eq!(i.color.unwrap().a, 1.0);
    assert_eq!(b.color.unwrap().a, 0.0);
    assert!(b.text_clip_fill.is_none());
}
