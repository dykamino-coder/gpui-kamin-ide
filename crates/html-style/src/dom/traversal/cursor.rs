//! Курсоры уже допущенного дерева: братья, inherited-состояние и отсечение поддеревьев.

use super::{Element, Node};

/// Прямой обход с состоянием родителя; хранятся только отложенные братья.
/// Цепочка без братьев не требует аллокаций. Ссылки на изменённые узлы не выходят.
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

/// Чтение с состоянием родителя. `None` отсекает детей, сохраняя братьев.
/// Хранятся только отложенные братья; цепочка без братьев не требует аллокаций.
pub fn read_inherited<'a, C: Clone>(
    nodes: &'a [Node],
    mut parent: C,
    mut enter: impl FnMut(&'a Element, &C) -> Option<C>,
) -> impl Iterator<Item = (&'a Node, C)> {
    let mut pending = Vec::new();
    let mut remaining = nodes;
    std::iter::from_fn(move || {
        loop {
            let Some((node, siblings)) = remaining.split_first() else {
                let (siblings, inherited) = pending.pop()?;
                remaining = siblings;
                parent = inherited;
                continue;
            };
            remaining = siblings;
            let inherited = parent.clone();
            if let Node::Element(element) = node
                && !element.children.is_empty()
                && let Some(next) = enter(element, &parent)
            {
                let previous = std::mem::replace(&mut parent, next);
                if !remaining.is_empty() {
                    pending.push((remaining, previous));
                }
                remaining = &element.children;
            }
            return Some((node, inherited));
        }
    })
}

/// Чтение в прямом порядке с сохранением порядка определений вызывающего.
pub fn read_nodes(nodes: &[Node]) -> impl Iterator<Item = &Node> {
    read_inherited(nodes, (), |_, _| Some(())).map(|(node, ())| node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_cursor_restores_owned_state_after_pruned_and_nested_branches() {
        let element = |tag: &str, children: Vec<Node>| {
            Node::Element(Element::synthetic(
                tag,
                Default::default(),
                Default::default(),
                children,
            ))
        };
        let nodes = vec![
            element(
                "root",
                vec![
                    element("branch", vec![element("leaf", vec![])]),
                    element("prune", vec![element("hidden", vec![])]),
                    element("tail", vec![]),
                ],
            ),
            element("after", vec![]),
        ];
        let visited = read_inherited(&nodes, String::new(), |e, parent| {
            (e.tag != "prune").then(|| format!("{parent}/{}", e.tag))
        })
        .map(|(node, parent)| match node {
            Node::Element(e) => (e.tag.as_str(), parent),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
        assert_eq!(
            visited,
            [
                ("root", ""),
                ("branch", "/root"),
                ("leaf", "/root/branch"),
                ("prune", "/root"),
                ("tail", "/root"),
                ("after", ""),
            ]
            .map(|(tag, parent)| (tag, parent.to_owned()))
        );
    }

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
