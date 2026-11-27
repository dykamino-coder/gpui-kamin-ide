//! Intrinsic dependency retries preserve author constraints and transaction boundaries.
use super::*;

#[test]
fn dependency_observer_is_optional_and_reports_native_retries() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static EVENTS: AtomicUsize = AtomicUsize::new(0);
    fn observe(event: crate::tree::LayoutDependencyEvent) {
        assert_eq!(event.kind, "layout.dependency_retry");
        assert!(event.own_gutter || event.descendant_gutter);
        EVENTS.fetch_add(1, Ordering::Relaxed);
    }
    let (mut tree, root, _, _, child) = intrinsic_fixture(false);
    tree.set_layout_observer(Some(observe));
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let recorded = EVENTS.load(Ordering::Relaxed);
    assert!(recorded > 0);
    tree.set_layout_observer(None);
    let mut style = tree.style(child).unwrap().clone();
    style.size.height = length(20.0);
    tree.set_style(child, style).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(EVENTS.load(Ordering::Relaxed), recorded);
}

#[test]
fn explicit_ancestor_width_survives_a_descendant_gutter_change() {
    let (mut tree, root, positioned, scroller, _) = intrinsic_fixture(false);
    let mut style = tree.style(positioned).unwrap().clone();
    style.size.width = length(40.0);
    tree.set_style(positioned, style).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(positioned).unwrap().size.width, 40.0);
    assert_eq!(tree.layout(scroller).unwrap().scrollbar_size.width, BAR);
}

#[test]
fn cached_layout_is_stable_and_a_later_transaction_can_remove_the_bar() {
    let (mut tree, root, positioned, scroller, child) = intrinsic_fixture(false);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let before = *tree.layout(positioned).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(*tree.layout(positioned).unwrap(), before);
    let mut style = tree.style(child).unwrap().clone();
    style.size.height = length(20.0);
    tree.set_style(child, style).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(scroller).unwrap().scrollbar_size.width, 0.0);
    assert_eq!(tree.layout(positioned).unwrap().size.width, 10.0);
}

#[test]
fn axis_query_does_not_publish_layout_or_visit_an_unrelated_tree() {
    let (mut tree, root, positioned, scroller, _) = intrinsic_fixture(false);
    let unrelated = tree.new_leaf_with_context(Style::default(), ()).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let before = [root, positioned, scroller].map(|id| *tree.layout(id).unwrap());
    tree.compute_size_with_measure(
        positioned,
        Size::MIN_CONTENT,
        crate::RequestedAxis::Horizontal,
        |input, node, _, style| {
            assert_ne!(node, unrelated);
            assert_eq!(input.run_mode, crate::RunMode::ComputeSize);
            crate::compute_leaf_layout(input, style, |_, _| 0.0, |_, _| Size::ZERO)
        },
    );
    assert_eq!(
        [root, positioned, scroller].map(|id| *tree.layout(id).unwrap()),
        before
    );
}

fn measured_chain(depth: usize) -> usize {
    let (mut tree, root, positioned, scroller, child) = intrinsic_fixture(false);
    let mut style = tree.style(child).unwrap().clone();
    style.size.width = auto();
    tree.set_style(child, style).unwrap();
    tree.set_node_context(child, Some(())).unwrap();
    tree.set_children(positioned, &[]).unwrap();
    let mut chain = scroller;
    for _ in 0..depth {
        chain = tree
            .new_with_children(
                Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                &[chain],
            )
            .unwrap();
    }
    tree.set_children(positioned, &[chain]).unwrap();
    let mut calls = 0;
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        calls += 1;
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, _| Size {
                width: known.width.unwrap_or(10.0),
                height: known.height.unwrap_or(80.0),
            },
        )
    })
    .unwrap();
    assert_eq!(tree.layout(positioned).unwrap().size.width, 25.0);
    calls
}

#[test]
fn a_deeper_intrinsic_chain_reuses_descendant_measurements() {
    let shallow = measured_chain(4);
    let deep = measured_chain(24);
    assert!(shallow > 0);
    assert!(
        deep <= shallow * 2,
        "measure calls: depth4={shallow}, depth24={deep}"
    );
}
