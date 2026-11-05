//! KaminIDE patch: the frame tree keeps nodes whose inputs did not change.

use super::{LayoutId, TaffyLayoutEngine};
use crate::{MeasuredContent, Pixels, Style, px, size};

fn leaf(engine: &mut TaffyLayoutEngine, width: f32) -> LayoutId {
    let mut style = Style::default();
    style.size.width = px(width).into();
    engine.request_layout(&style, px(16.), 1.0, &[])
}

fn parent(engine: &mut TaffyLayoutEngine, children: &[LayoutId]) -> LayoutId {
    engine.request_layout(&Style::default(), px(16.), 1.0, children)
}

fn measured(engine: &mut TaffyLayoutEngine, key: Option<u64>) -> LayoutId {
    if let Some(key) = key {
        engine.set_next_layout_key(key);
    }
    engine.request_measured_layout_with_box(Style::default(), px(16.), 1.0, |_, _, _, _, _| {
        MeasuredContent::from((size(Pixels(10.), Pixels(10.)), None, None, None))
    })
}

#[test]
fn an_unchanged_pass_keeps_every_node() {
    let mut engine = TaffyLayoutEngine::new();
    let (a, b) = (leaf(&mut engine, 10.), leaf(&mut engine, 20.));
    let p = parent(&mut engine, &[a, b]);
    engine.clear();
    let (a2, b2) = (leaf(&mut engine, 10.), leaf(&mut engine, 20.));
    assert_eq!((a2, b2), (a, b));
    assert_eq!(parent(&mut engine, &[a2, b2]), p);
    assert_eq!(engine.claimed_nodes(), 3);
}

#[test]
fn a_changed_leaf_renews_it_and_its_ancestors_and_drops_the_old_ones() {
    let mut engine = TaffyLayoutEngine::new();
    let (a, b) = (leaf(&mut engine, 10.), leaf(&mut engine, 20.));
    let p = parent(&mut engine, &[a, b]);
    let root = parent(&mut engine, &[p]);
    engine.clear();
    let (a2, b2) = (leaf(&mut engine, 10.), leaf(&mut engine, 30.));
    let p2 = parent(&mut engine, &[a2, b2]);
    let root2 = parent(&mut engine, &[p2]);
    assert_eq!(a2, a);
    // Only the unchanged leaf is kept; the others reuse storage as new nodes.
    assert_eq!(engine.claimed_nodes(), 1);
    let _ = (b, p, root, root2);
    engine.clear();
    assert_eq!(engine.taffy.total_node_count(), 4);
    assert_eq!(engine.taffy.children_slice(p2.0), &[a2.0, b2.0]);
}

#[test]
fn a_measured_leaf_is_kept_only_under_an_honoured_equal_key() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let keyed = measured(&mut engine, Some(7));
    let plain = measured(&mut engine, None);
    engine.clear();
    engine.set_layout_keys(true);
    assert_eq!(measured(&mut engine, Some(7)), keyed);
    let _ = (plain, measured(&mut engine, None));
    assert_eq!(engine.claimed_nodes(), 1);
    engine.clear();
    // Keys outside `set_layout_keys(true)` are ignored.
    measured(&mut engine, Some(7));
    assert_eq!(engine.claimed_nodes(), 1);
}

#[test]
fn a_parent_with_other_children_is_not_kept() {
    let mut engine = TaffyLayoutEngine::new();
    let (a, b) = (leaf(&mut engine, 10.), leaf(&mut engine, 20.));
    let p = parent(&mut engine, &[a, b]);
    engine.clear();
    let a2 = leaf(&mut engine, 10.);
    assert_eq!(a2, a);
    assert_ne!(parent(&mut engine, &[a2]), p);
    assert_eq!(engine.claimed_nodes(), 1);
}

#[test]
fn a_kept_node_takes_the_shared_style_of_its_pass() {
    let mut engine = TaffyLayoutEngine::new();
    // More distinct styles than sharing candidates: the first one is forgotten.
    let first = leaf(&mut engine, 0.);
    for width in 1..=super::style_share::CANDIDATES {
        leaf(&mut engine, width as f32);
    }
    let old = engine.taffy.shared_style(first.0).unwrap();
    engine.clear();
    assert_eq!(leaf(&mut engine, 0.), first);
    let twin = leaf(&mut engine, 0.);
    assert_ne!(twin, first);
    let kept = engine.taffy.shared_style(first.0).unwrap();
    assert!(taffy::SharedStyle::ptr_eq(&kept, &engine.taffy.shared_style(twin.0).unwrap()));
    assert!(!taffy::SharedStyle::ptr_eq(&kept, &old));
    assert_eq!(*kept, *old);
}

#[test]
fn a_leaf_whose_style_changed_is_not_kept() {
    let mut engine = TaffyLayoutEngine::new();
    let a = leaf(&mut engine, 10.);
    engine.clear();
    let a2 = leaf(&mut engine, 11.);
    assert_eq!(engine.claimed_nodes(), 0);
    assert_eq!(engine.taffy.style(a2.0).unwrap().size.width, taffy::Dimension::length(11.));
    let _ = a;
}
#[test]
fn a_subtree_of_another_size_before_a_keyed_leaf_shifts_no_later_leaf() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let _ = (leaf(&mut engine, 10.), leaf(&mut engine, 20.));
    let keyed = measured(&mut engine, Some(7));
    let after = leaf(&mut engine, 30.);
    engine.clear();
    // Two leaves become one: the plain leaf after the keyed one keeps its
    // node although its position in the request sequence moved.
    engine.set_layout_keys(true);
    let _ = leaf(&mut engine, 99.);
    assert_eq!(measured(&mut engine, Some(7)), keyed);
    assert_eq!(leaf(&mut engine, 30.), after);
    assert_eq!(engine.claimed_nodes(), 2);
}
#[test]
fn of_equal_keys_the_leaf_nearest_its_position_is_kept() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let first = measured(&mut engine, Some(7));
    let _ = leaf(&mut engine, 10.);
    let second = measured(&mut engine, Some(7));
    let after = leaf(&mut engine, 20.);
    engine.clear();
    // The first subtree became one leaf: the second keyed leaf is nearer its
    // previous position than the first one, and the leaf after it follows.
    engine.set_layout_keys(true);
    let _ = leaf(&mut engine, 99.);
    assert_eq!(measured(&mut engine, Some(7)), second);
    assert_eq!(leaf(&mut engine, 20.), after);
    let _ = first;
}
#[test]
fn a_new_plain_request_does_not_take_a_later_keyed_leaf() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let keyed = measured(&mut engine, Some(7));
    let after = leaf(&mut engine, 20.);
    engine.clear();
    // A new leaf before them sits at the keyed leaf's old position.
    engine.set_layout_keys(true);
    let _ = leaf(&mut engine, 99.);
    assert_eq!(measured(&mut engine, Some(7)), keyed);
    assert_eq!(leaf(&mut engine, 20.), after);
}
/// A table cell: a plain leaf, a keyed text leaf and their parent.
fn cell(engine: &mut TaffyLayoutEngine, key: u64) -> [LayoutId; 3] {
    let plain = leaf(engine, 10.);
    let keyed = measured(engine, Some(key));
    [plain, keyed, parent(engine, &[plain, keyed])]
}

#[test]
fn a_leaf_whose_new_key_a_later_leaf_has_takes_no_node_of_a_later_request() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let before = [1, 2, 3, 4].map(|key| cell(&mut engine, key));
    parent(&mut engine, &before.map(|c| c[2]));
    engine.clear();
    // The second cell's text became the fourth one's: the second cell is new,
    // and the cells after it keep their nodes, the fourth its own leaf.
    engine.set_layout_keys(true);
    let after = [1, 4, 3, 4].map(|key| cell(&mut engine, key));
    parent(&mut engine, &after.map(|c| c[2]));
    assert_eq!([after[0], after[2], after[3]], [before[0], before[2], before[3]]);
    assert_eq!(after[1][0], before[1][0]);
    assert_eq!(engine.claimed_nodes(), 10);
}
