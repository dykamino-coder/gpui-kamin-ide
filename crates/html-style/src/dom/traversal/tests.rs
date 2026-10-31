//! Прямой/обратный порядок, наследование и COW глубокого DOM.

use super::*;
use crate::budget::admission::Admission;

fn element(tag: &str, children: Vec<Node>) -> Node {
    Node::Element(Element::synthetic(
        tag,
        Default::default(),
        Default::default(),
        children,
    ))
}

#[test]
fn sibling_order_and_inherited_context_are_stable() {
    let mut tree = vec![
        element("a", vec![element("b", vec![]), element("c", vec![])]),
        element("d", vec![]),
    ];
    let mut events = Vec::new();
    preorder(
        &Admission::default(),
        &mut tree,
        String::new(),
        |e, parent| {
            let path = format!("{parent}{}", e.tag);
            events.push(path.clone());
            Some(path)
        },
    )
    .unwrap();
    assert_eq!(events, ["a", "ab", "ac", "d"]);
    events.clear();
    postorder_inherited(
        &Admission::default(),
        &mut tree,
        String::new(),
        |e, parent| format!("{parent}{}", e.tag),
        |e, parent| {
            events.push(format!("{parent}{}", e.tag));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(events, ["ab", "ac", "a", "d"]);
    assert_eq!(
        nodes(&Admission::default(), &tree, |_| true)
            .map(Result::unwrap)
            .filter_map(|n| match n {
                Node::Element(e) => Some(e.tag.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["a", "b", "c", "d"]
    );
}

#[test]
fn pruning_visits_the_next_sibling_and_postorder_sees_restored_children() {
    let mut tree = vec![
        element("a", vec![element("b", vec![])]),
        element("c", vec![]),
    ];
    let mut events = Vec::new();
    preorder(&Admission::default(), &mut tree, (), |e, _| {
        events.push(e.tag.clone());
        None
    })
    .unwrap();
    assert_eq!(events, ["a", "c"]);
    postorder(&Admission::default(), &mut tree, |e| {
        if e.tag == "b" {
            e.tag = "changed".into();
        }
        if e.tag == "a" {
            assert!(matches!(&e.children[0], Node::Element(e) if e.tag == "changed"));
        }
    })
    .unwrap();
}

#[test]
fn deep_walks_and_cow_drop_use_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let mut tree = vec![Node::Text("KEEP".into())];
            for _ in 0..10_000 {
                tree = vec![element("div", tree)];
            }
            let copy = tree.clone();
            let mut visits = 0;
            preorder(&Admission::default(), &mut tree, 0usize, |e, &depth| {
                e.node_id = depth as u64 + 1;
                visits += 1;
                Some(depth + 1)
            })
            .unwrap();
            assert_eq!(visits, 10_000);
            assert!(matches!(&copy[0], Node::Element(e) if e.node_id == 0));
            visits = 0;
            postorder(&Admission::default(), &mut tree, |_| visits += 1).unwrap();
            assert_eq!(visits, 10_000);
            assert_eq!(
                nodes(&Admission::default(), &tree, |_| true).count(),
                10_001
            );
            drop(copy);
            drop(tree);
        })
        .unwrap()
        .join()
        .unwrap();
}
