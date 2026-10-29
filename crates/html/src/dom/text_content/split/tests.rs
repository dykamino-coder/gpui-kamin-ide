//! Порядок разреза, UTF-8, пустые элементы и глубокий DOM.
use super::*;
fn element(tag: &str, children: Vec<Node>) -> Node {
    Node::Element(Element::synthetic(
        tag,
        Default::default(),
        Default::default(),
        children,
    ))
}
fn text(nodes: &[Node]) -> String {
    let mut out = String::new();
    crate::dom::text_content::gather_text(nodes, &mut out);
    out
}
#[test]
fn split_keeps_siblings_empty_nodes_and_utf8() {
    let nodes = vec![
        element("b", vec![Node::Text("aé".into()), element("i", vec![])]),
        Node::Text("z".into()),
    ];
    let admission = Admission::default();
    let (head, tail) = split_nodes(&admission, &nodes, 2).unwrap();
    assert_eq!((text(&head), text(&tail)), ("aé".into(), "z".into()));
    assert!(
        matches!(&tail[0], Node::Element(e) if matches!(&e.children[0], Node::Element(e) if e.tag == "i"))
    );
    let (head, tail) = split_nodes(&admission, &nodes, 3).unwrap();
    assert!(matches!(&head[0], Node::Element(e) if e.children.len() == 2));
    assert!(matches!(&tail[0], Node::Text(t) if t == "z"));
}
#[test]
fn deep_split_uses_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let mut nodes = vec![Node::Text("KEEPLOST".into())];
            for _ in 0..10_000 {
                nodes = vec![element("span", nodes)];
            }
            let (head, tail) = split_nodes(&Admission::default(), &nodes, 4).unwrap();
            assert_eq!(text(&head), "KEEP");
            assert_eq!(text(&tail), "LOST");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn split_at_nested_sibling_boundary_counts_the_remaining_text() {
    let nodes = vec![
        element(
            "b",
            vec![
                Node::Text("abc".into()),
                element("i", vec![Node::Text("def".into())]),
            ],
        ),
        Node::Text("ghi".into()),
    ];
    let (head, tail) = split_nodes(&Admission::default(), &nodes, 3).unwrap();
    assert_eq!((text(&head), text(&tail)), ("abc".into(), "defghi".into()));
    assert!(matches!(&head[0], Node::Element(e) if e.children.len() == 1));
    assert!(matches!(&tail[0], Node::Element(e) if e.children.len() == 1));
}
