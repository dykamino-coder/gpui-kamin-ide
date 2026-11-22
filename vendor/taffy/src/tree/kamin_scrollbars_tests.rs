//! KaminIDE patch: tests of the bars layout decides (`kamin_scrollbars`).
use crate::prelude::*;
use crate::geometry::Point;
use crate::style::{Overflow, RootGutter, ScrollbarLayout};

const BAR: f32 = 15.0;

fn scroller(tree: &mut TaffyTree, guess: bool, viewport: bool, children: &[NodeId]) -> NodeId {
    let mode = if guess { Overflow::Scroll } else { Overflow::Hidden };
    tree.new_with_children(
        Style {
            display: Display::Block,
            size: Size { width: length(200.0), height: length(100.0) },
            overflow: Point { x: Overflow::Hidden, y: mode },
            scrollbar_width: BAR,
            kamin_scrollbars: ScrollbarLayout::new(Point { x: false, y: true }, viewport, false, None),
            ..Default::default()
        },
        children,
    )
    .unwrap()
}

fn block(tree: &mut TaffyTree, height: f32) -> NodeId {
    tree.new_leaf(Style { display: Display::Block, size: Size { width: auto(), height: length(height) }, ..Default::default() })
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
            size: Size { width: length(200.0), height: length(500.0) },
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
            inset: Rect { left: auto(), right: length(BAR), top: length(0.0), bottom: auto() },
            size: Size { width: length(20.0), height: length(20.0) },
            kamin_scrollbars: ScrollbarLayout::new(
                Point { x: false, y: false },
                false,
                false,
                Some(RootGutter {
                    assumed: RootGutter::sides(Rect { left: 0.0, right: BAR, top: 0.0, bottom: 0.0 }),
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
            Style { size: Size { width: length(200.0), height: length(100.0) }, ..Default::default() },
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
                size: Size { width: length(200.0), height: length(100.0) },
                overflow: Point { x: Overflow::Hidden, y: Overflow::Hidden },
                scrollbar_width: BAR,
                kamin_scrollbars: ScrollbarLayout::new(Point { x: true, y: true }, false, false, None),
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
                let line = Size { width: 500.0, height: 20.0 };
                crate::MeasureOutput { overflow: Some(line), ..Size { width, height: 20.0 }.into() }
            },
        )
    })
    .unwrap();
    assert_eq!(tree.style(root).unwrap().overflow, Point { x: Overflow::Scroll, y: Overflow::Hidden });
}
