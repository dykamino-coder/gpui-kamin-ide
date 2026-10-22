//! Реестр calc разделяется с копией дерева при расчёте повторяемых дорожек.
use crate::prelude::*;
use crate::tree::calc_handle;

#[test]
fn registered_calc_repeat_survives_measurement_tree_copy() {
    let source: TaffyTree<()> = TaffyTree::new();
    let handle = {
        let _scope = source.calc_scope();
        calc_handle(20.0, 0.25)
    };
    for repetition in [RepetitionCount::AutoFill, RepetitionCount::AutoFit] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.share_calc_registry(&source);
        let children: Vec<_> = (0..2)
            .map(|_| {
                tree.new_leaf(Style {
                    size: Size {
                        width: length(1.0),
                        height: length(10.0),
                    },
                    ..Style::DEFAULT
                })
                .unwrap()
            })
            .collect();
        let grid = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    size: Size {
                        width: length(200.0),
                        height: length(20.0),
                    },
                    grid_template_columns: vec![repeat(
                        repetition,
                        vec![minmax(MinTrackSizingFunction::calc(handle), fr(1.0))],
                    )],
                    gap: length(10.0),
                    ..Style::DEFAULT
                },
                &children,
            )
            .unwrap();
        tree.compute_layout(grid, Size::MAX_CONTENT).unwrap();
        assert_eq!(tree.layout(children[1]).unwrap().location.x, 105.0);
        tree.mark_dirty(grid).unwrap();
        tree.compute_layout(grid, Size::MAX_CONTENT).unwrap();
        assert_eq!(tree.layout(children[1]).unwrap().location.x, 105.0);
    }
}
