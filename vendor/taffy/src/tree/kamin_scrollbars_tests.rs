//! KaminIDE patch: tests of the bars layout decides (`kamin_scrollbars`).
use crate::geometry::Point;
use crate::prelude::*;
use crate::style::{Overflow, RootGutter, ScrollbarLayout};

const BAR: f32 = 15.0;

fn scroller(tree: &mut TaffyTree, guess: bool, viewport: bool, children: &[NodeId]) -> NodeId {
    let mode = if guess {
        Overflow::Scroll
    } else {
        Overflow::Hidden
    };
    tree.new_with_children(
        Style {
            display: Display::Block,
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            overflow: Point {
                x: Overflow::Hidden,
                y: mode,
            },
            scrollbar_width: BAR,
            kamin_scrollbars: ScrollbarLayout::new(
                Point { x: false, y: true },
                viewport,
                false,
                None,
            ),
            ..Default::default()
        },
        children,
    )
    .unwrap()
}

fn block(tree: &mut TaffyTree, height: f32) -> NodeId {
    tree.new_leaf(Style {
        display: Display::Block,
        size: Size {
            width: auto(),
            height: length(height),
        },
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn content_that_overflows_gets_a_bar_and_a_narrower_box_in_the_same_layout() {
    let mut tree = TaffyTree::new();
    let tall = block(&mut tree, 300.0);
    let root = scroller(&mut tree, false, false, &[tall]);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.style(root).unwrap().overflow.y, Overflow::Scroll);
    assert_eq!(tree.layout(tall).unwrap().size.width, 200.0 - BAR);
    assert_eq!(tree.layout(root).unwrap().scrollbar_size.width, BAR);
}

#[test]
fn a_wrong_guess_of_a_bar_is_taken_back_in_the_same_layout() {
    let mut tree = TaffyTree::new();
    let short = block(&mut tree, 40.0);
    let root = scroller(&mut tree, true, false, &[short]);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.style(root).unwrap().overflow.y, Overflow::Hidden);
    assert_eq!(tree.layout(short).unwrap().size.width, 200.0);
    assert_eq!(tree.layout(root).unwrap().scrollbar_size.width, 0.0);
}

#[test]
fn a_proxy_does_not_keep_the_bar_of_a_range_measured_before() {
    let mut tree = TaffyTree::new();
    let short = block(&mut tree, 40.0);
    let proxy = tree
        .new_leaf(Style {
            position: Position::Absolute,
            size: Size {
                width: length(200.0),
                height: length(500.0),
            },
            kamin_scrollbars: ScrollbarLayout::new(Point { x: false, y: false }, false, true, None),
            ..Default::default()
        })
        .unwrap();
    let root = scroller(&mut tree, true, false, &[proxy, short]);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.style(root).unwrap().overflow.y, Overflow::Hidden);
}

#[test]
fn boxes_built_for_the_guessed_root_gutter_follow_the_gutter_the_viewport_decides() {
    let mut tree = TaffyTree::new();
    let short = block(&mut tree, 40.0);
    // Built for the guessed vertical bar: the right inset already stops at the gutter.
    let fixed = tree
        .new_leaf(Style {
            position: Position::Absolute,
            inset: Rect {
                left: auto(),
                right: length(BAR),
                top: length(0.0),
                bottom: auto(),
            },
            size: Size {
                width: length(20.0),
                height: length(20.0),
            },
            kamin_scrollbars: ScrollbarLayout::new(
                Point { x: false, y: false },
                false,
                false,
                Some(RootGutter {
                    assumed: RootGutter::sides(Rect {
                        left: 0.0,
                        right: BAR,
                        top: 0.0,
                        bottom: 0.0,
                    }),
                    grow: false,
                }),
            ),
            ..Default::default()
        })
        .unwrap();
    let viewport = scroller(&mut tree, true, true, &[short]);
    // The box hangs on a box around the viewport, whose area knows no gutter.
    let root = tree
        .new_with_children(
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            &[viewport, fixed],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.style(viewport).unwrap().overflow.y, Overflow::Hidden);
    // No bar: the author's `right: 0` reaches the viewport's edge, as built without one.
    assert_eq!(tree.layout(fixed).unwrap().location.x, 200.0 - 20.0);
}

#[test]
fn lines_longer_than_their_box_bring_the_horizontal_bar() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let text = tree.new_leaf_with_context(Style::default(), ()).unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                overflow: Point {
                    x: Overflow::Hidden,
                    y: Overflow::Hidden,
                },
                scrollbar_width: BAR,
                kamin_scrollbars: ScrollbarLayout::new(
                    Point { x: true, y: true },
                    false,
                    false,
                    None,
                ),
                ..Default::default()
            },
            &[text],
        )
        .unwrap();
    // A preformatted line of 500 px in a box the measure keeps at its width.
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, available| {
                let width = match available.width {
                    AvailableSpace::Definite(width) => width.min(500.0),
                    _ => 500.0,
                };
                let line = Size {
                    width: 500.0,
                    height: 20.0,
                };
                crate::MeasureOutput {
                    overflow: Some(line),
                    ..Size {
                        width,
                        height: 20.0,
                    }
                    .into()
                }
            },
        )
    })
    .unwrap();
    assert_eq!(
        tree.style(root).unwrap().overflow,
        Point {
            x: Overflow::Scroll,
            y: Overflow::Hidden
        }
    );
}

#[path = "kamin_scrollbars_dependency_tests.rs"]
mod dependency_tests;

/// The bars layout settles on depend on the content only, not on the bars the
/// box was built with: a box whose content fits the scrollport exactly in one
/// axis keeps no bar there whatever the guess (CSS Overflow 3 §3, `auto`).
#[test]
fn bars_settle_to_the_same_fixed_point_from_every_guess() {
    for guess in [Point { x: false, y: false }, Point { x: true, y: true }, Point { x: false, y: true }, Point { x: true, y: false }] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let cover = tree
            .new_leaf(Style {
                position: Position::Absolute,
                inset: Rect { left: length(0.0), right: length(0.0), top: length(0.0), bottom: length(0.0) },
                ..Default::default()
            })
            .unwrap();
        let tall = tree
            .new_leaf(Style {
                display: Display::Flex,
                margin: Rect { left: zero(), right: zero(), top: length(20.2), bottom: zero() },
                size: Size { width: length(129.3), height: length(1584.8) },
                ..Default::default()
            })
            .unwrap();
        let mode = |bar: bool| if bar { Overflow::Scroll } else { Overflow::Hidden };
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    size: Size { width: length(176.286_71), height: length(32.0) },
                    overflow: Point { x: mode(guess.x), y: mode(guess.y) },
                    scrollbar_width: BAR,
                    kamin_scrollbars: ScrollbarLayout::new(Point { x: true, y: true }, false, false, None),
                    ..Default::default()
                },
                &[cover, tall],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.style(root).unwrap().overflow,
            Point { x: Overflow::Hidden, y: Overflow::Scroll },
            "guess {guess:?}"
        );
    }
}
/// A shrink-to-fit box widens by the vertical bar that appears in its layout
/// (CSS 2.1 §10.3.7, the gutter in its intrinsic inline size): the horizontal
/// bar is decided at the width it settles on, not at the narrower width it had
/// before its parent took the new gutter, and the bars do not depend on the
/// guess they were built with (Blink `PaintLayerScrollableArea::UpdateAfterLayout`
/// sets `SetIntrinsicLogicalWidthsDirty` and decides again at the new size).
#[test]
fn a_shrink_to_fit_box_decides_its_other_bar_at_the_width_its_gutter_gives_it() {
    for guess in [Point { x: false, y: false }, Point { x: true, y: true }, Point { x: false, y: true }, Point { x: true, y: false }] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let content = tree
            .new_leaf(Style {
                display: Display::Block,
                size: Size { width: length(161.286_71), height: length(1621.0) },
                ..Default::default()
            })
            .unwrap();
        let mode = |bar: bool| if bar { Overflow::Scroll } else { Overflow::Hidden };
        let scroller = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    position: Position::Absolute,
                    size: Size { width: auto(), height: length(32.0) },
                    max_size: Size { width: length(200.0), height: length(390.0) },
                    overflow: Point { x: mode(guess.x), y: mode(guess.y) },
                    scrollbar_width: BAR,
                    kamin_scrollbars: ScrollbarLayout::new(Point { x: true, y: true }, false, false, None),
                    ..Default::default()
                },
                &[content],
            )
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    size: Size { width: length(800.0), height: length(600.0) },
                    ..Default::default()
                },
                &[scroller],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.style(scroller).unwrap().overflow,
            Point { x: Overflow::Hidden, y: Overflow::Scroll },
            "guess {guess:?}"
        );
        // The tree rounds its layout to whole pixels.
        assert_eq!(tree.layout(scroller).unwrap().size.width, (161.286_71 + BAR).round(), "guess {guess:?}");
    }
}