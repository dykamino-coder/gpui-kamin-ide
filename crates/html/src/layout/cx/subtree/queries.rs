//! Полные запросы paint к фактам снимка; отложенная обёртка использует курсор.

use crate::dom::{Element, Node, traversal::read_nodes};
use crate::layout::cx::FrameCx;

// Владение записью кэша остаётся здесь: потребители запрашивают только факт.
enum Query<'a> {
    BreaksAbsent(&'a Element),
    FloatsAbsent(&'a Element),
    BlendsInside(&'a [Node]),
    PositionedInside(&'a Element),
}

fn cached(query: Query<'_>, frame: FrameCx<'_>) -> Option<bool> {
    frame.subtrees.map(|facts| {
        let mut facts = facts.borrow_mut();
        match query {
            Query::BreaksAbsent(e) => facts.breaks_absent(e),
            Query::FloatsAbsent(e) => facts.floats_absent(e),
            Query::BlendsInside(nodes) => facts.blends_inside(nodes),
            Query::PositionedInside(e) => facts.positioned_inside(e),
        }
    })
}

/// Неизвестное поддерево нельзя считать свободным от разрывов.
pub(crate) fn breaks_absent(root: &Element, frame: FrameCx<'_>) -> bool {
    cached(Query::BreaksAbsent(root), frame).unwrap_or(false)
}

/// Неизвестное поддерево нельзя считать свободным от плавающих коробок.
pub(crate) fn floats_absent(root: &Element, frame: FrameCx<'_>) -> bool {
    cached(Query::FloatsAbsent(root), frame).unwrap_or(false)
}

/// CSS Compositing: blending-потомок требует изоляции своего контекста.
/// Граница вложенного контекста не прерывает это консервативное доказательство.
pub(crate) fn blends_inside(nodes: &[Node], frame: FrameCx<'_>) -> bool {
    if let Some(value) = cached(Query::BlendsInside(nodes), frame) {
        return value;
    }
    read_nodes(nodes).any(|node| match node {
        Node::Element(e) => e.style.used().reset_effects.blend.is_some_and(|b| b != 0),
        Node::Text(_) => false,
    })
}

/// CSS2 Appendix E: позиционированный потомок сохраняет порядок соседних слоёв.
pub(crate) fn positioned_inside(node: &Node, frame: FrameCx<'_>) -> bool {
    let Node::Element(e) = node else { return false };
    if let Some(value) = cached(Query::PositionedInside(e), frame) {
        return value;
    }
    read_nodes(std::slice::from_ref(node)).any(|node| match node {
        Node::Element(e) => e.style.used().reset_positioned.position.is_some(),
        Node::Text(_) => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;
    use crate::layout::table::anon::anon_element;
    use crate::style::computed::{Computed, Position, ResetEffects, ResetPositioned, StyleGroup};
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn deep_paint_queries_agree_with_and_without_snapshot_facts() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let mut leaf = anon_element("span", vec![]);
                leaf.style = Computed {
                    reset_effects: StyleGroup::new(ResetEffects {
                        blend: Some(1),
                        ..Default::default()
                    }),
                    reset_positioned: StyleGroup::new(ResetPositioned {
                        position: Some(Position::Relative),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
                .into();
                let mut root = leaf;
                for _ in 0..20_000 {
                    root = anon_element("div", vec![Node::Element(root)]);
                }
                let nodes = vec![Node::Element(root)];
                let doc = DocState::new(Default::default());
                let plain = FrameCx::new(&doc);
                assert!(blends_inside(&nodes, plain));
                assert!(positioned_inside(&nodes[0], plain));
                let facts = Rc::new(RefCell::new(Default::default()));
                let mut cached = plain;
                cached.subtrees = Some(&facts);
                assert!(blends_inside(&nodes, cached));
                assert!(positioned_inside(&nodes[0], cached));
                let mut next = nodes.as_slice();
                while let Some(Node::Element(e)) = next.first() {
                    assert!(blends_inside(next, cached));
                    assert!(positioned_inside(&next[0], cached));
                    next = &e.children;
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
