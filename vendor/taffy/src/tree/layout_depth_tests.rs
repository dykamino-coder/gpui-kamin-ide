//! Полная раскладка и вложенные callbacks не расходуют стек по глубине.
use super::*;
use crate::Dimension;

fn small_stack(run: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

fn nested_layout(display: Display) {
    small_stack(move || {
        let mut tree = TaffyTree::<()>::new();
        let style = SharedStyle::new(Style {
            display,
            size: Size {
                width: Dimension::length(32.0),
                height: Dimension::length(16.0),
            },
            ..Style::default()
        });
        let leaf = tree.new_leaf(style.clone()).unwrap();
        let mut root = leaf;
        for _ in 0..20_000 {
            root = tree.new_with_children(style.clone(), &[root]).unwrap();
        }
        let mut measures = 0;
        tree.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
            |input, _, _, style| {
                measures += 1;
                compute_leaf_layout(input, style, calc_value, |_, _| Size::ZERO)
            },
        )
        .unwrap();
        assert!(measures > 0);
        assert_eq!(
            tree.layout(root).unwrap().size,
            Size {
                width: 32.0,
                height: 16.0
            }
        );
        assert_eq!(
            tree.layout(leaf).unwrap().size,
            Size {
                width: 32.0,
                height: 16.0
            }
        );
        assert!(tree.nodes.values().all(|n| !n.cache.is_empty()));
        let first_measures = measures;
        tree.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
            |input, _, _, style| {
                measures += 1;
                compute_leaf_layout(input, style, calc_value, |_, _| Size::ZERO)
            },
        )
        .unwrap();
        assert_eq!(
            measures, first_measures,
            "a cached root does not repeat leaf measurement"
        );
    });
}

#[test]
fn block_layout_twenty_thousand_levels_on_small_stack() {
    nested_layout(Display::Block);
}
#[test]
fn flex_layout_twenty_thousand_levels_on_small_stack() {
    nested_layout(Display::Flex);
}
#[test]
fn grid_layout_twenty_thousand_levels_on_small_stack() {
    nested_layout(Display::Grid);
}

struct NestedMeasure<'a> {
    descendants: &'a mut [(TaffyTree<()>, NodeId)],
    calls: &'a mut usize,
}
impl crate::tree::LayoutMeasurer<()> for NestedMeasure<'_> {
    async fn measure<'a>(
        &'a mut self,
        input: LayoutInput,
        _: NodeId,
        _: Option<&'a mut ()>,
        style: &'a Style,
        stack: &'a mut reblessive::Stk,
    ) -> LayoutOutput {
        *self.calls += 1;
        let measured = match self.descendants.split_first_mut() {
            None => Size {
                width: 11.0,
                height: 7.0,
            },
            Some(((tree, root), descendants)) => {
                stack
                    .run(|stack| {
                        tree.compute_layout_with_measure_async(
                            *root,
                            Size {
                                width: AvailableSpace::MaxContent,
                                height: AvailableSpace::MaxContent,
                            },
                            NestedMeasure {
                                descendants,
                                calls: self.calls,
                            },
                            stack,
                        )
                    })
                    .await
                    .unwrap();
                tree.layout(*root).unwrap().size
            }
        };
        compute_leaf_layout(input, style, calc_value, |_, _| measured)
    }
}

#[test]
fn lending_measurements_share_the_stack_at_twenty_thousand_levels() {
    small_stack(|| {
        let mut trees: Vec<_> = (0..20_000)
            .map(|_| {
                let mut tree = TaffyTree::<()>::new();
                let root = tree.new_leaf(Style::default()).unwrap();
                (tree, root)
            })
            .collect();
        let ((tree, root), descendants) = trees.split_first_mut().unwrap();
        let mut calls = 0;
        reblessive::Stack::new()
            .enter(|stack| {
                tree.compute_layout_with_measure_async(
                    *root,
                    Size {
                        width: AvailableSpace::MaxContent,
                        height: AvailableSpace::MaxContent,
                    },
                    NestedMeasure {
                        descendants,
                        calls: &mut calls,
                    },
                    stack,
                )
            })
            .finish()
            .unwrap();
        assert_eq!(calls, 20_000);
        assert_eq!(
            tree.layout(*root).unwrap().size,
            Size {
                width: 11.0,
                height: 7.0
            }
        );
    });
}
