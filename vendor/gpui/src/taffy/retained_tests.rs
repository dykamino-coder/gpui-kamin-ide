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
    for width in 1..=9 {
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
fn a_kept_subtree_keeps_its_stamp_and_memo_and_a_renewed_one_loses_them() {
    let mut engine = TaffyLayoutEngine::new();
    let a = leaf(&mut engine, 10.);
    let p = parent(&mut engine, &[a]);
    let stamp = engine.layout_stamp(p);
    assert!(stamp.is_some());
    engine.set_layout_memo(p, std::rc::Rc::new(5u32));
    engine.clear();
    let a2 = leaf(&mut engine, 10.);
    let p2 = parent(&mut engine, &[a2]);
    assert_eq!((p2, engine.layout_stamp(p2)), (p, stamp));
    let memo = engine.layout_memo(p2).expect("memo of a kept node");
    assert_eq!(memo.downcast_ref::<u32>(), Some(&5));
    engine.clear();
    let b = leaf(&mut engine, 30.);
    let p3 = parent(&mut engine, &[b]);
    assert_ne!(engine.layout_stamp(p3), stamp);
    assert!(engine.layout_memo(p3).is_none());
}

#[test]
fn a_capped_child_is_restored_and_kept_by_the_next_equal_request() {
    let mut engine = TaffyLayoutEngine::new();
    let a = leaf(&mut engine, 10.);
    let p = parent(&mut engine, &[a]);
    assert!(engine.cap_child_outer_width(p, 0, px(4.), 1.0));
    engine.clear();
    let a2 = leaf(&mut engine, 10.);
    assert_eq!(parent(&mut engine, &[a2]), p);
    assert_eq!(engine.claimed_nodes(), 2);
}

#[test]
fn leaves_of_one_key_are_kept_at_their_positions_with_their_parents() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let mut parents = Vec::new();
    for width in [10., 20., 30., 40.] {
        let probe = measured(&mut engine, Some(1));
        let a = leaf(&mut engine, width);
        parents.push(parent(&mut engine, &[probe, a]));
    }
    engine.clear();
    engine.set_layout_keys(true);
    for (width, old) in [10., 20., 30., 40.].into_iter().zip(parents) {
        let probe = measured(&mut engine, Some(1));
        let a = leaf(&mut engine, width);
        assert_eq!(parent(&mut engine, &[probe, a]), old);
    }
    assert_eq!(engine.claimed_nodes(), 12);
}

#[test]
fn a_claimed_leaf_leaves_the_candidates_of_its_key() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let a = measured(&mut engine, Some(1));
    let _ = leaf(&mut engine, 10.);
    let b = measured(&mut engine, Some(1));
    engine.clear();
    engine.set_layout_keys(true);
    // `a` at its position, `b` from the candidates (its position holds the plain leaf).
    assert_eq!(measured(&mut engine, Some(1)), a);
    assert_eq!(measured(&mut engine, Some(1)), b);
    assert!(engine.retained.by_key.get(&1).is_none_or(|list| list.is_empty()));
    for leaf in [a, b] {
        let context = engine.taffy.get_node_context(leaf.0).expect("measured leaf");
        assert_eq!(context.key_slot, super::retained::NO_SLOT);
    }
    // A third request of the key finds no claimed leaf to take again.
    let c = measured(&mut engine, Some(1));
    assert!(c != a && c != b);
    assert_eq!(engine.claimed_nodes(), 2);
}
fn constant(engine: &mut TaffyLayoutEngine, class: u16) -> LayoutId {
    engine.set_next_layout_constant(std::num::NonZeroU16::new(class).unwrap());
    measured(engine, None)
}

#[test]
fn constant_leaves_are_kept_at_their_positions_without_side_tables() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let mut parents = Vec::new();
    for width in [10., 20., 30.] {
        let probe = constant(&mut engine, 1);
        let a = leaf(&mut engine, width);
        parents.push(parent(&mut engine, &[probe, a]));
    }
    engine.clear();
    assert!(engine.retained.by_key.is_empty());
    engine.set_layout_keys(true);
    for (width, old) in [10., 20., 30.].into_iter().zip(parents) {
        let probe = constant(&mut engine, 1);
        let a = leaf(&mut engine, width);
        assert_eq!(parent(&mut engine, &[probe, a]), old);
    }
    assert_eq!(engine.claimed_nodes(), 9);
    engine.clear();
    engine.set_layout_keys(true);
    // Another class, or a keyed request, does not take a constant leaf.
    let other = constant(&mut engine, 2);
    let _ = measured(&mut engine, Some(1));
    assert_eq!(engine.claimed_nodes(), 9);
    let _ = other;
}

#[test]
fn a_stamped_node_is_never_reused_for_another_subtree() {
    let mut engine = TaffyLayoutEngine::new();
    let a = leaf(&mut engine, 10.);
    let p = parent(&mut engine, &[a]);
    let stamp = engine.layout_stamp(p);
    let l = leaf(&mut engine, 5.);
    let plain = parent(&mut engine, &[l]);
    engine.clear();
    // Changed subtrees: the stamped parent is not recycled, the plain one is.
    let l = leaf(&mut engine, 30.);
    let p2 = parent(&mut engine, &[l]);
    let l = leaf(&mut engine, 6.);
    let q2 = parent(&mut engine, &[l]);
    assert_ne!(p2, p);
    assert_eq!(q2, plain);
    assert_ne!(engine.layout_stamp(p2), stamp);
    engine.clear();
    // The swept stamped node's id never comes back.
    for _ in 0..4 {
        let l = leaf(&mut engine, 7.);
        let n = parent(&mut engine, &[l]);
        engine.clear();
        assert_ne!(engine.layout_stamp(n), stamp);
    }
}