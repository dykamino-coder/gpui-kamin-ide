//! Min-content constraints suppress flexible track expansion in either axis.
use crate::geometry::Size;
use crate::prelude::*;

fn grid(tree: &mut TaffyTree<()>, vertical: bool) -> NodeId {
    let children: Vec<_> = [50.0, 50.0, 0.0]
        .into_iter()
        .map(|extent| {
            tree.new_leaf(Style {
                size: if vertical {
                    Size {
                        width: length(100.0),
                        height: Dimension::length(extent),
                    }
                } else {
                    Size {
                        width: Dimension::length(extent),
                        height: length(100.0),
                    }
                },
                ..Style::DEFAULT
            })
            .unwrap()
        })
        .collect();
    let tracks = vec![fr(1.0); 3];
    tree.new_with_children(
        Style {
            display: Display::Grid,
            grid_template_columns: if vertical {
                vec![auto()]
            } else {
                tracks.clone()
            },
            grid_template_rows: if vertical { tracks } else { vec![auto()] },
            flex_grow: 1.0,
            ..Style::DEFAULT
        },
        &children,
    )
    .unwrap()
}

#[test]
fn min_content_does_not_equalize_flexible_tracks() {
    for vertical in [false, true] {
        for (constraint, expected) in [
            (AvailableSpace::MinContent, 100.0),
            (AvailableSpace::MaxContent, 150.0),
        ] {
            let mut tree = TaffyTree::new();
            let node = grid(&mut tree, vertical);
            tree.compute_layout(
                node,
                if vertical {
                    Size {
                        width: AvailableSpace::Definite(100.0),
                        height: constraint,
                    }
                } else {
                    Size {
                        width: constraint,
                        height: AvailableSpace::Definite(100.0),
                    }
                },
            )
            .unwrap();
            let size = tree.layout(node).unwrap().size;
            assert_eq!(if vertical { size.height } else { size.width }, expected);
        }
    }
}

#[test]
fn column_flex_auto_minimum_preserves_empty_fractional_row() {
    let mut tree = TaffyTree::new();
    let child = grid(&mut tree, true);
    let root = tree
        .new_with_children(
            Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(child).unwrap().size.height, 100.0);
    let empty = tree.children(child).unwrap()[2];
    assert_eq!(tree.layout(empty).unwrap().size.height, 0.0);
}
