//! KaminIDE patch: the leaf of an unpainted wrapper stands in for the wrapper
//! and its content at every parent width at which the content is the same.

use super::{LayoutId, TaffyLayoutEngine};
use taffy::prelude::*;

/// A row wrapper (stretched, inflexible) around a fixed `content` x 40 box.
fn wrapper(engine: &mut TaffyLayoutEngine, content: f32) -> NodeId {
    let inner = engine
        .taffy
        .new_leaf(Style {
            size: Size { width: Dimension::length(content), height: Dimension::length(40.0) },
            flex_shrink: 0.0,
            ..Default::default()
        })
        .unwrap();
    engine
        .taffy
        .new_with_children(
            // GPUI boxes are relatively positioned (gpui::Style default).
            Style {
                display: Display::Flex,
                position: Position::Relative,
                flex_shrink: 0.0,
                ..Default::default()
            },
            &[inner],
        )
        .unwrap()
}

/// A column flow of `width` with a 10 px block before `child`.
fn flow(engine: &mut TaffyLayoutEngine, width: f32, child: NodeId) -> NodeId {
    let before = engine
        .taffy
        .new_leaf(Style {
            size: Size { width: Dimension::auto(), height: Dimension::length(10.0) },
            flex_shrink: 0.0,
            ..Default::default()
        })
        .unwrap();
    let root = engine
        .taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size { width: Dimension::length(width), height: Dimension::auto() },
                ..Default::default()
            },
            &[before, child],
        )
        .unwrap();
    engine.taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    root
}

/// What the parent and the following content see: the child's place and
/// height, and the parent's scrollable overflow.
fn seen(engine: &TaffyLayoutEngine, root: NodeId, child: NodeId) -> (f32, f32, f32, f32) {
    let child = engine.taffy.layout(child).unwrap();
    let root = engine.taffy.layout(root).unwrap();
    let overflow = root.scrollable_overflow_rect;
    (child.location.x, child.location.y, child.size.height, overflow.right.max(root.size.width))
}

#[test]
fn an_overflowing_unpainted_wrapper_is_replaced_at_any_narrower_width() {
    let mut engine = TaffyLayoutEngine::new();
    let wrap = wrapper(&mut engine, 500.0);
    flow(&mut engine, 300.0, wrap);
    let id = LayoutId::from(wrap);
    assert!(engine.exact_leaf(id).is_none(), "the content overflows the wrapper");
    let leaf = engine.unpainted_leaf(id).expect("an unpainted wrapper qualifies");
    assert_eq!(leaf.width(), 500.0);
    for width in [300.0, 200.0, 499.0] {
        let mut real = TaffyLayoutEngine::new();
        let wrap = wrapper(&mut real, 500.0);
        let root = flow(&mut real, width, wrap);
        let mut replaced = TaffyLayoutEngine::new();
        let stand_in: NodeId = replaced.request_exact_leaf(&leaf).into();
        let replaced_root = flow(&mut replaced, width, stand_in);
        assert_eq!(seen(&real, root, wrap), seen(&replaced, replaced_root, stand_in), "{width}");
    }
}

#[test]
fn a_fitting_unpainted_wrapper_takes_the_flow_width() {
    let mut engine = TaffyLayoutEngine::new();
    let wrap = wrapper(&mut engine, 100.0);
    flow(&mut engine, 300.0, wrap);
    let leaf = engine.unpainted_leaf(LayoutId::from(wrap)).unwrap();
    assert_eq!(leaf.width(), 300.0);
    let mut replaced = TaffyLayoutEngine::new();
    let stand_in: NodeId = replaced.request_exact_leaf(&leaf).into();
    flow(&mut replaced, 200.0, stand_in);
    let layout = replaced.taffy.layout(stand_in).unwrap();
    assert_eq!((layout.size.width, layout.size.height), (200.0, 40.0));
}
