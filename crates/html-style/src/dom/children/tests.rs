//! Копирование и освобождение глубоких общих поддеревьев.

use super::*;

#[test]
fn published_snapshot_releases_builder_capacity_and_still_detaches_on_growth() {
    let mut nodes = Vec::with_capacity(64);
    nodes.push(Node::Text("original".into()));
    let mut children = Children::from(nodes);
    assert_eq!(children.capacity(), children.len());
    let copy = children.clone();
    assert_eq!(children.identity(), copy.identity());
    children.push(Node::Text("new".into()));
    assert_ne!(children.identity(), copy.identity());
    assert_eq!(copy.capacity(), 1);
    assert_eq!(copy.len(), 1);
    assert_eq!(children.len(), 2);
    assert!(matches!(&copy[0], Node::Text(text) if text == "original"));
}

#[test]
fn mutations_detach_only_the_changed_list() {
    let mut children: Children = vec![Node::Text("original".into())].into();
    let copy = children.clone();
    children.push(Node::Text("new".into()));
    assert_eq!(copy.len(), 1);
    assert_eq!(children.len(), 2);
    assert!(matches!(&copy[0], Node::Text(t) if t == "original"));
    assert_eq!(copy.into_vec().len(), 1);
}

#[test]
fn deep_shared_subtrees_drop_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let mut nodes = vec![Node::Text("leaf".into())];
            for _ in 0..10_000 {
                nodes = vec![Node::Element(crate::dom::Element::synthetic(
                    "div",
                    Default::default(),
                    Default::default(),
                    nodes,
                ))];
            }
            let copy = nodes.clone();
            drop(nodes);
            drop(copy);
        })
        .unwrap()
        .join()
        .unwrap();
}
