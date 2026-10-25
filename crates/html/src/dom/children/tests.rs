//! Копирование и освобождение глубоких общих поддеревьев.

use super::*;

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
                nodes = vec![Node::Element(crate::layout::table::anon::anon_element(
                    "div", nodes,
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
