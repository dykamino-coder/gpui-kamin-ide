//! Unchanged normalization shares its retained source allocation.

use super::*;

#[test]
fn unchanged_text_owns_one_buffer_and_transformed_text_keeps_the_source_intact() {
    let mut tree = BoxTree::default();
    let nodes = [Node::Text("plain text".into())];
    tree.publish(&nodes);
    let id = tree.flow(&nodes).unwrap();
    let source = tree.get(id).unwrap().inline.as_ref().unwrap().clone();
    let style = BoxStyle::default();
    let text = tree.normalized_inline(id, &style).unwrap();
    assert!(
        Rc::ptr_eq(&source, &text),
        "unchanged text must share its buffer"
    );

    let mut uppercase = style.clone();
    uppercase.inh.text_flow.text_transform = Some(crate::style::computed::TextTransform::Upper);
    tree.prepare(id, &uppercase);
    let transformed = tree.normalized_inline(id, &uppercase).unwrap();
    assert_eq!(transformed.text, "PLAIN TEXT");
    assert_eq!(source.text, "plain text");
    assert!(!Rc::ptr_eq(&source, &transformed));
    assert!(tree.normalized_inline(id, &style).is_none());
}
