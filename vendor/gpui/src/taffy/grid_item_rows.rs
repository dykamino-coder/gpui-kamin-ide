//! Точные границы рядов элемента из завершённого размещения родительской сетки.
use taffy::{
    BoxGenerationMode, CoreStyle, DetailedLayoutInfo, Display, NodeId, Position, TaffyTree,
    TraversePartialTree,
};

pub(super) fn grid_item_rows<C>(tree: &TaffyTree<C>, child: NodeId) -> Option<(u16, u16)> {
    let parent = tree.parent(child)?;
    if tree.style(parent).ok()?.display != Display::Grid {
        return None;
    }
    tree.computed_layout_output(parent).ok()??;
    let DetailedLayoutInfo::Grid(info) = tree.detailed_layout_info(parent) else {
        return None;
    };
    // Grid восстанавливает source_order после subgrid и исключает сплющенных потомков.
    let index = tree
        .child_ids(parent)
        .filter(|&node| {
            tree.style(node).is_ok_and(|style| {
                style.box_generation_mode() != BoxGenerationMode::None
                    && style.position != Position::Absolute
            })
        })
        .position(|node| node == child)?;
    let item = info.items.get(index)?;
    Some((item.row_start, item.row_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::*;

    #[test]
    fn short_spanning_item_uses_placement_with_hidden_and_oof_siblings() {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let hidden = tree
            .new_leaf(Style {
                display: Display::None,
                ..Style::DEFAULT
            })
            .unwrap();
        let absolute = tree
            .new_leaf(Style {
                position: Position::Absolute,
                ..Style::DEFAULT
            })
            .unwrap();
        let short = tree
            .new_leaf(Style {
                size: Size::from_lengths(5.0, 2.0),
                align_self: Some(AlignItems::START),
                grid_row: Line {
                    start: line(1),
                    end: line(4),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let other = tree
            .new_leaf(Style {
                grid_row: Line {
                    start: line(2),
                    end: line(3),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    grid_template_rows: vec![length(20.0); 3],
                    grid_template_columns: vec![length(20.0)],
                    ..Style::DEFAULT
                },
                &[hidden, absolute, short, other],
            )
            .unwrap();
        assert_eq!(grid_item_rows(&tree, short), None);
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(tree.layout(short).unwrap().size.height, 2.0);
        assert_eq!(grid_item_rows(&tree, short), Some((1, 4)));
        assert_eq!(grid_item_rows(&tree, other), Some((2, 3)));
        assert_eq!(grid_item_rows(&tree, hidden), None);
        assert_eq!(grid_item_rows(&tree, absolute), None);
        tree.set_children(root, &[other, hidden, short, absolute])
            .unwrap();
        assert_eq!(grid_item_rows(&tree, short), None);
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(grid_item_rows(&tree, short), Some((1, 4)));
        assert_eq!(grid_item_rows(&tree, other), Some((2, 3)));
        tree.set_style(root, Style::DEFAULT).unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(grid_item_rows(&tree, short), None);
    }

    #[test]
    fn leading_implicit_tracks_and_auto_placement_use_native_indices() {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let automatic = tree.new_leaf(Style::DEFAULT).unwrap();
        let before = tree
            .new_leaf(Style {
                grid_row: Line {
                    start: line(-4),
                    end: line(-3),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let explicit = tree
            .new_leaf(Style {
                grid_row: Line {
                    start: line(1),
                    end: line(2),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    grid_template_rows: vec![length(20.0); 2],
                    grid_template_columns: vec![length(20.0)],
                    grid_auto_rows: vec![length(10.0)],
                    ..Style::DEFAULT
                },
                &[automatic, before, explicit],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(grid_item_rows(&tree, before), Some((1, 2)));
        assert_eq!(grid_item_rows(&tree, explicit), Some((2, 3)));
        assert_eq!(grid_item_rows(&tree, automatic), Some((3, 4)));
    }

    #[test]
    fn subgrid_descendants_do_not_shift_direct_child_metadata() {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let nested = [
            tree.new_leaf(Style::DEFAULT).unwrap(),
            tree.new_leaf(Style::DEFAULT).unwrap(),
        ];
        let subgrid = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    subgrid: taffy::style::SUBGRID_ROWS,
                    grid_row: Line {
                        start: line(2),
                        end: line(4),
                    },
                    grid_template_columns: vec![length(20.0)],
                    ..Style::DEFAULT
                },
                &nested,
            )
            .unwrap();
        let other = tree
            .new_leaf(Style {
                grid_row: Line {
                    start: line(1),
                    end: line(2),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    grid_template_rows: vec![length(20.0); 3],
                    grid_template_columns: vec![length(20.0)],
                    ..Style::DEFAULT
                },
                &[subgrid, other],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(grid_item_rows(&tree, subgrid), Some((2, 4)));
        assert_eq!(grid_item_rows(&tree, other), Some((1, 2)));
    }
}
