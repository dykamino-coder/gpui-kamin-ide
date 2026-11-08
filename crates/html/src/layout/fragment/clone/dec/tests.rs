//! Клонированные декорации фрагмента и допустимость потомков.

use super::*;

#[test]
fn border_box_clone_subtracts_edges_once_and_does_not_reapply_minimum() {
    let tree = crate::document::parsed::parse(
        "<div style='height:100px;min-height:80px;max-height:120px;box-sizing:border-box;padding:10px;box-decoration-break:clone'></div>",
        "",
    );
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    let fragment = clone_fragment(find(&tree).unwrap(), 10.0, 10.0, 30.0, 50.0, 30.0);
    assert_eq!(
        fragment.style.used().reset_box_layout.height,
        Some(Len::Px(30.0))
    );
    assert_eq!(fragment.style.used().reset_box_layout.min_height, None);
    let Node::Element(clip) = &fragment.children[0] else {
        panic!("clip box")
    };
    let Node::Element(body) = &clip.children[0] else {
        panic!("body box")
    };
    let style = body.style.used();
    assert_eq!(style.reset_box_layout.height, Some(Len::Px(80.0)));
    assert_eq!(style.reset_box_layout.min_height, Some(Len::Px(60.0)));
    assert_eq!(style.reset_box_layout.max_height, Some(Len::Px(100.0)));
    assert_eq!(style.reset_positioned.inset.top, Some(Len::Px(-30.0)));
    assert_eq!(style.reset_border.border_box, Some(false));
}

// Допустимость потомков для клонированных декораций не ограничена глубиной.
use crate::style::computed::{
    Position, ResetBoxLayout, ResetFragmentation, ResetPositioned, Sides, StyleGroup,
};

fn element(position: Option<Position>, children: Vec<Node>) -> Element {
    Element::synthetic(
        "div",
        Computed {
            reset_positioned: StyleGroup::new(ResetPositioned {
                position,
                ..Default::default()
            }),
            ..Default::default()
        },
        Default::default(),
        children,
    )
}

fn decorated(children: Vec<Node>) -> Element {
    Element::synthetic(
        "div",
        Computed {
            reset_fragmentation: StyleGroup::new(ResetFragmentation {
                bdb_clone: true,
                ..Default::default()
            }),
            reset_box_layout: StyleGroup::new(ResetBoxLayout {
                padding: Sides {
                    top: Some(Len::Px(1.0)),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        },
        Default::default(),
        children,
    )
}

#[test]
fn deep_clone_decoration_checks_descendant_position_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            for position in [
                None,
                Some(Position::Relative),
                Some(Position::Absolute),
                Some(Position::Fixed),
            ] {
                let mut nodes = vec![Node::Element(element(position, vec![]))];
                for _ in 0..20_000 {
                    nodes = vec![Node::Element(element(None, nodes))];
                }
                let root = decorated(nodes);
                assert_eq!(
                    clone_dec(&root),
                    if matches!(position, Some(Position::Absolute) | Some(Position::Fixed)) {
                        None
                    } else {
                        Some((1.0, 0.0))
                    }
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
