//! Тесты: анонимные ruby-контейнеры сохраняют границы пробелов и авторское блочное состояние.

use super::*;

#[test]
fn misparented_annotations_keep_only_internal_whitespace() {
    let annotation = |text: &str| {
        Node::Element(Element::anonymous(
            "rt",
            Computed::default(),
            vec![Node::Text(text.into())],
            true,
            Marks::default(),
        ))
    };
    let nodes = wrap_misparented_ruby(
        vec![
            Node::Text(" ".into()),
            annotation("a"),
            Node::Text("\t".into()),
            annotation("b"),
            Node::Text(" ".into()),
        ],
        None,
    );
    assert_eq!(nodes.len(), 3);
    assert!(matches!(&nodes[0], Node::Text(t) if t == " "));
    assert!(matches!(&nodes[2], Node::Text(t) if t == " "));
    let Node::Element(ruby) = &nodes[1] else {
        panic!("ruby wrapper")
    };
    assert_eq!(ruby.tag, "ruby");
    assert!(ruby.inline);
    assert_eq!(ruby.node_id, 0);
    assert_eq!(ruby.children.len(), 3);
    assert!(matches!(&ruby.children[1], Node::Text(t) if t == "\t"));
    assert!(ruby.attrs.is_empty());
    assert_eq!(ruby.marks, Marks::default());
}

#[test]
fn block_ruby_keeps_authored_properties_on_outer_box() {
    fn authored_ruby(nodes: &[Node]) -> Option<&Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.attr("id") == Some("source") {
                    return Some(e);
                }
                if let Some(found) = authored_ruby(&e.children) {
                    return Some(found);
                }
            }
        }
        None
    }
    let tree = parse(
        r#"<ruby id="source" style="display:block ruby;color:red">base<rt>note</rt></ruby>"#,
        "",
    );
    let outer = authored_ruby(&tree).expect("authored ruby");
    assert!(outer.node_id > 0);
    assert_eq!(
        outer.style.display,
        Some(crate::style::computed::Display::Block)
    );
    assert_eq!(outer.children.len(), 1);
    let Node::Element(inner) = &outer.children[0] else {
        panic!("inline ruby")
    };
    assert_eq!(inner.tag, "ruby");
    assert!(inner.inline);
    assert_eq!(inner.node_id, 0);
    assert_eq!(inner.style.color, None);
    assert!(outer.style.color.is_some());
    assert!(inner.attrs.is_empty());
    // Идентичность коробки не меняет её layout-флаги.
    let mut marks = inner.marks.clone();
    assert!(marks.state_id.get().is_some());
    marks.state_id = Default::default();
    assert_eq!(marks, Marks::default());
    assert_eq!(inner.children.len(), 2);
}
