//! Cursors for passes over an already admitted tree; no additional work charges.

use super::{Element, Node};

/// Preorder with parent state, saving only pending siblings. A single-child
/// chain needs no traversal allocation. No references to changed nodes escape.
pub fn for_each_inherited<C>(
    nodes: &mut [Node],
    mut parent: C,
    mut enter: impl FnMut(&mut Element, &C) -> C,
) {
    let mut pending = Vec::new();
    let mut remaining = nodes;
    loop {
        let Some((node, siblings)) = remaining.split_first_mut() else {
            let Some((siblings, inherited)) = pending.pop() else {
                break;
            };
            remaining = siblings;
            parent = inherited;
            continue;
        };
        remaining = siblings;
        let Node::Element(element) = node else {
            continue;
        };
        let next = enter(element, &parent);
        if !element.children.is_empty() {
            if !remaining.is_empty() {
                pending.push((remaining, parent));
            }
            remaining = &mut element.children;
            parent = next;
        }
    }
}

/// Read nodes in preorder, preserving first/last definition behavior of callers.
pub fn read_nodes(nodes: &[Node]) -> impl Iterator<Item = &Node> {
    let mut pending = Vec::new();
    let mut remaining = nodes;
    std::iter::from_fn(move || {
        loop {
            let Some((node, siblings)) = remaining.split_first() else {
                remaining = pending.pop()?;
                continue;
            };
            remaining = siblings;
            if let Node::Element(element) = node
                && !element.children.is_empty()
            {
                if !remaining.is_empty() {
                    pending.push(remaining);
                }
                remaining = &element.children;
            }
            return Some(node);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_cursor_restores_parent_state_for_pending_siblings() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let element = |tag: &str, children: Vec<Node>| {
                    Node::Element(Element::synthetic(
                        tag,
                        Default::default(),
                        Default::default(),
                        children,
                    ))
                };
                let mut nodes = vec![element("leaf", vec![])];
                for _ in 0..20_000 {
                    nodes = vec![element("div", nodes)];
                }
                nodes.push(element("tail", vec![]));
                let mut nodes = vec![element("root", nodes), element("after", vec![])];
                let mut visited = Vec::new();
                for_each_inherited(&mut nodes, 0, |e, parent| {
                    if e.tag != "div" {
                        visited.push((e.tag.clone(), *parent));
                    }
                    parent + 1
                });
                assert_eq!(
                    visited,
                    [("root", 0), ("leaf", 20_001), ("tail", 1), ("after", 0)]
                        .map(|(tag, depth)| (tag.into(), depth))
                );
                assert_eq!(read_nodes(&nodes).count(), 20_004);
                assert_eq!(
                    read_nodes(&nodes)
                        .filter_map(|n| match n {
                            Node::Element(e) if e.tag != "div" => Some(e.tag.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    ["root", "leaf", "tail", "after"]
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
