//! KaminIDE patch: borrowed OOF candidates retain ownership and hoisted-list lifecycle.

use super::taffy_tree::TaffyView;
use crate::style_helpers::*;
use crate::{
    compute_oof_layout_for_area, AxisStaticEdge, AxisStaticPosition, ContainingBlockClaims,
    Direction, OofCandidate, OofCandidates, OofPositioningArea, Point, Position, Size, Style,
    TaffyTree,
};

#[test]
fn explicit_area_can_reuse_borrowed_candidates_without_consuming_the_wrapper() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let child = tree
        .new_leaf(Style {
            position: Position::Absolute,
            size: Size {
                width: length(20.0),
                height: length(10.0),
            },
            ..Style::DEFAULT
        })
        .unwrap();
    let owner = tree.new_with_children(Style::DEFAULT, &[child]).unwrap();
    let mut candidates = OofCandidates::new();
    candidates.push(OofCandidate {
        node: child,
        order: 0,
        position: Position::Absolute,
        static_position: Point {
            x: AxisStaticPosition::from_edge(7.0, AxisStaticEdge::Start),
            y: AxisStaticPosition::from_edge(9.0, AxisStaticEdge::Start),
        },
    });
    let original = candidates.clone();
    for _ in 0..2 {
        let mut view = TaffyView {
            taffy: &mut tree,
            measure_function: |input, _, _, style| {
                crate::compute_leaf_layout(input, style, |_, _| 0.0, |_, _| Size::ZERO)
            },
        };
        let output = compute_oof_layout_for_area(
            &mut view,
            owner,
            candidates.as_slice(),
            OofPositioningArea {
                size: Size {
                    width: 100.0,
                    height: 80.0,
                },
                offset: Point::ZERO,
            },
            Direction::Ltr,
            ContainingBlockClaims::ALL,
        );
        assert_eq!(output.hoisted, vec![child]);
        assert!(output.unclaimed.is_empty());
        assert_eq!(
            tree.unrounded_layout(child).location,
            Point { x: 7.0, y: 9.0 }
        );
        assert_eq!(candidates, original);
    }
}

#[test]
fn nested_fixed_candidate_is_cleared_after_removal_and_relayout() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let fixed = tree
        .new_leaf(Style {
            position: Position::Fixed,
            ..Style::DEFAULT
        })
        .unwrap();
    let absolute = tree
        .new_with_children(
            Style {
                position: Position::Absolute,
                ..Style::DEFAULT
            },
            &[fixed],
        )
        .unwrap();
    let root = tree.new_with_children(Style::DEFAULT, &[absolute]).unwrap();
    for _ in 0..2 {
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(tree.hoisted_children(root).unwrap(), &[absolute, fixed]);
    }
    tree.set_children(root, &[]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert!(tree.hoisted_children(root).unwrap().is_empty());
}
