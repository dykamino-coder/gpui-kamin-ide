//! Border snapping follows resolved lengths and invalidates retained computed styles.

use super::tests::element;
use super::*;
use crate::style::values::value::Len;

#[test]
fn borders_recompute_from_cascade_after_scale_and_style_changes() {
    let mut parsed = crate::dom::parse(
        r#"<body><div style="border:1px solid"><p style="border:inherit">x</p></div></body>"#,
        "",
    );
    let parent = Rc::new(Computed::default());
    for (scale, expected) in [(1.0, 1.0), (1.25, 0.8), (2.0, 1.0), (1.25, 0.8)] {
        parsed.values().doc.set_device_scale(scale);
        materialize(&parsed.nodes, &parent, parsed.values());
        for tag in ["div", "p"] {
            let style = &element(&parsed.nodes, tag).style;
            assert_eq!(style.computed().borders().left, Some(Len::Px(expected)));
        }
        assert_eq!(
            element(&parsed.nodes, "div")
                .style
                .cascaded()
                .borders()
                .left,
            Some(Len::Px(1.0))
        );
    }
    fn edit(nodes: &mut [Node]) {
        for node in nodes {
            if let Node::Element(e) = node {
                if e.tag == "div" {
                    e.style.cascaded_mut().reset_border.border_width.left = Some(Len::Px(2.0));
                    return;
                }
                edit(&mut e.children);
            }
        }
    }
    edit(&mut parsed.nodes);
    materialize(&parsed.nodes, &parent, parsed.values());
    for tag in ["div", "p"] {
        assert_eq!(
            element(&parsed.nodes, tag).style.computed().borders().left,
            Some(Len::Px(1.6))
        );
    }
}

#[test]
fn relative_and_physical_border_widths_snap_after_zoom() {
    let parsed = crate::dom::parse(
        r#"<body><div style="font-size:10px;zoom:1.5;border:0.1em solid;border-left-width:2px">x</div></body>"#,
        "",
    );
    parsed.values().doc.set_device_scale(1.25);
    materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        parsed.values(),
    );
    let style = element(&parsed.nodes, "div").style.computed();
    assert_eq!(style.borders().top, Some(Len::Px(0.8)));
    assert_eq!(style.borders().left, Some(Len::Px(2.4)));
}
