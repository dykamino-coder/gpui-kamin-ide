//! Границы правил из настоящих полос строк, включая перенос и усечение row-gap.

use super::*;
use gpui::{FlexItemLayout, FlexLineLayout};

fn ids() -> [LayoutId; 4] {
    std::array::from_fn(|index| taffy::NodeId::from(index).into())
}

fn item(id: LayoutId, a: f32, b: f32) -> FlexItemLayout {
    FlexItemLayout {
        id,
        order: 0,
        main_offset: b,
        cross_offset: a,
        main_size: 40.0,
        cross_size: 50.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
    }
}

fn runs(lines: &FlexLines) -> (Vec<super::super::GapRun>, Vec<super::super::GapRun>) {
    super::super::line_runs(
        &[],
        Some(0.0),
        None,
        None,
        (false, false),
        Some((1, 0.0, 90.0)),
        false,
        None,
        Some(lines),
    )
}

#[test]
fn carried_row_gap_keeps_its_center_and_suppressed_gap_keeps_its_slot() {
    for remaining in [10.0, 4.0, 0.0] {
        use taffy::prelude::*;
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let mut child = Style {
            size: Size::from_lengths(40.0, 50.0),
            flex_shrink: 0.0,
            ..Style::DEFAULT
        };
        let first = tree.new_leaf(child.clone()).unwrap();
        child.margin.top = LengthPercentageAuto::length(remaining);
        let second = tree.new_leaf(child).unwrap();
        let root = tree
            .new_with_children(
                Style {
                    size: Size {
                        width: Dimension::length(40.0),
                        height: Dimension::auto(),
                    },
                    flex_wrap: FlexWrap::Wrap,
                    ..Style::DEFAULT
                },
                &[first, second],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let taffy::DetailedLayoutInfo::Flex(native) = tree.detailed_layout_info(root) else {
            panic!("native flex metadata")
        };
        let layout = FlexLayout {
            is_column: false,
            lines: native
                .lines
                .iter()
                .map(|line| FlexLineLayout {
                    cross_offset: line.cross_offset,
                    cross_size: line.cross_size,
                    items: line
                        .items
                        .iter()
                        .map(|i| item(i.node.into(), i.cross_offset, i.main_offset))
                        .collect(),
                })
                .collect(),
        };
        let lines = FlexLines::from_layout(
            (0.0, 0.0),
            &layout,
            &HashMap::from([(second.into(), remaining)]),
        );
        let (main, _) = runs(&lines);
        assert_eq!(main.len(), 1);
        assert_eq!((main[0].g0 + main[0].g1) / 2.0, 50.0 + remaining / 2.0);
        assert_eq!((main[0].index, main[0].count), (0, 1));
    }
}

#[test]
fn line_boundaries_include_alignment_space_instead_of_item_extents() {
    let ids = ids();
    let layout = FlexLayout {
        is_column: false,
        lines: vec![
            FlexLineLayout {
                cross_offset: 0.0,
                cross_size: 80.0,
                items: vec![item(ids[0], 20.0, 0.0)],
            },
            FlexLineLayout {
                cross_offset: 100.0,
                cross_size: 80.0,
                items: vec![item(ids[1], 120.0, 0.0)],
            },
        ],
    };
    let lines = FlexLines::from_layout((3.0, 7.0), &layout, &HashMap::new());
    let (main, _) = runs(&lines);
    assert_eq!((main[0].g0, main[0].g1), (87.0, 107.0));
}

#[test]
fn overlapping_items_do_not_merge_distinct_native_lines() {
    let ids = ids();
    let mut overflowing = item(ids[0], 0.0, 0.0);
    overflowing.cross_size = 120.0;
    let layout = FlexLayout {
        is_column: false,
        lines: vec![
            FlexLineLayout {
                cross_offset: 0.0,
                cross_size: 50.0,
                items: vec![overflowing, item(ids[1], 0.0, 50.0)],
            },
            FlexLineLayout {
                cross_offset: 60.0,
                cross_size: 50.0,
                items: vec![item(ids[2], 60.0, 0.0), item(ids[3], 60.0, 50.0)],
            },
        ],
    };
    let lines = FlexLines::from_layout((0.0, 0.0), &layout, &HashMap::new());
    let (main, cross) = runs(&lines);
    assert_eq!(main.len(), 1);
    assert_eq!(cross.len(), 2);
    assert_eq!((cross[0].r0, cross[0].r1), (0.0, 50.0));
    assert_eq!((cross[1].r0, cross[1].r1), (60.0, 110.0));
}

#[test]
fn physical_sort_preserves_native_membership_for_columns_and_nan() {
    let ids = ids();
    let layout = FlexLayout {
        is_column: true,
        lines: vec![
            FlexLineLayout {
                cross_offset: 60.0,
                cross_size: 50.0,
                items: vec![item(ids[0], 60.0, 8.0)],
            },
            FlexLineLayout {
                cross_offset: f32::NAN,
                cross_size: 50.0,
                items: vec![],
            },
            FlexLineLayout {
                cross_offset: 0.0,
                cross_size: 50.0,
                items: vec![item(ids[1], 0.0, 12.0)],
            },
        ],
    };
    let lines = FlexLines::from_layout((3.0, 7.0), &layout, &HashMap::new());
    assert_eq!(&lines.lines[..2], &[(3.0, 53.0), (63.0, 113.0)]);
    assert_eq!(lines.members[0][0].b0, 19.0);
    assert_eq!(lines.members[1][0].b0, 15.0);
    assert!(lines.lines[2].0.is_nan());
}
