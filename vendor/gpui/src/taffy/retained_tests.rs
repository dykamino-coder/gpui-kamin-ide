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
fn a_captured_subtree_is_claimed_whole_while_its_root_keeps_the_stamp() {
    let mut engine = TaffyLayoutEngine::new();
    // A request before the subtree: the pass that claims it first has the
    // subtree at another place of its sequence.
    let _ = leaf(&mut engine, 5.);
    let start = engine.request_position();
    let (a, b) = (leaf(&mut engine, 10.), leaf(&mut engine, 20.));
    let p = parent(&mut engine, &[a, b]);
    let mut subtree = engine.capture_subtree(start, p).expect("captured");
    let nodes = [a.0, b.0, p.0];
    assert_eq!(engine.subtree_nodes(&subtree), Some(&nodes[..]));
    let captured = subtree;
    let root = parent(&mut engine, &[p]);
    engine.clear();
    // Claimed whole: no requests for its nodes, the next pass keeps them.
    assert_eq!(engine.claim_subtree(&mut subtree), Some(p));
    // The claim renews the capture: the same nodes at their place in this pass.
    assert_eq!(engine.subtree_nodes(&subtree), Some(&nodes[..]));
    assert_eq!(engine.claim_subtree(&mut subtree), None, "claimed once per pass");
    assert_eq!(engine.claim_subtree(&mut { captured }), None, "claimed once per pass");
    assert_eq!(parent(&mut engine, &[p]), root);
    assert!(!engine.layout_stale());
    engine.clear();
    assert_eq!(engine.taffy.total_node_count(), 4);
    // The capture of the first pass names nothing two passes later; the
    // renewed one is claimed again.
    assert_eq!(engine.claim_subtree(&mut { captured }), None);
    assert_eq!(engine.claim_subtree(&mut subtree), Some(p));
    assert_eq!(engine.subtree_nodes(&subtree), Some(&nodes[..]));
    engine.clear();
    // A pass that renews the subtree drops the stamp: the capture is void.
    let (a2, c) = (leaf(&mut engine, 10.), leaf(&mut engine, 30.));
    let _ = parent(&mut engine, &[a2, c]);
    engine.clear();
    assert_eq!(engine.claim_subtree(&mut subtree), None);
}

#[test]
fn a_capture_is_not_read_from_another_engine_or_after_the_tree_is_forgotten() {
    let mut engine = TaffyLayoutEngine::new();
    let start = engine.request_position();
    let a = leaf(&mut engine, 10.);
    let p = parent(&mut engine, &[a]);
    let subtree = engine.capture_subtree(start, p).expect("captured");
    // Another engine with the same requests has nodes of the same ids.
    let mut other = TaffyLayoutEngine::new();
    let a2 = leaf(&mut other, 10.);
    let p2 = parent(&mut other, &[a2]);
    let _ = other.layout_stamp(p2);
    other.clear();
    assert_eq!(other.claim_subtree(&mut { subtree }), None);
    assert!(other.subtree_nodes(&subtree).is_none());
    engine.clear();
    engine.forget();
    assert_eq!(engine.claim_subtree(&mut { subtree }), None);
}

#[test]
fn a_keyed_leaf_of_a_subtree_claimed_whole_leaves_the_candidates_of_its_key() {
    let mut engine = TaffyLayoutEngine::new();
    engine.set_layout_keys(true);
    let start = engine.request_position();
    let m = measured(&mut engine, Some(1));
    let p = parent(&mut engine, &[m]);
    let mut subtree = engine.capture_subtree(start, p).expect("captured");
    engine.clear();
    engine.set_layout_keys(true);
    assert_eq!(engine.claim_subtree(&mut subtree), Some(p));
    assert!(engine.retained.by_key.get(&1).is_none_or(|list| list.is_empty()));
    let context = engine.taffy.get_node_context(m.0).expect("measured leaf");
    assert_eq!(context.key_slot, super::retained::NO_SLOT);
    // A request of the same key does not take the claimed leaf again.
    assert_ne!(measured(&mut engine, Some(1)), m);
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
    // A new leaf first: the cursor is astray, so the requests take the
    // nearest candidates, `b` ahead and then `a` behind.
    let _ = leaf(&mut engine, 99.);
    assert_eq!(measured(&mut engine, Some(1)), b);
    assert_eq!(measured(&mut engine, Some(1)), a);
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

fn cell(engine: &mut TaffyLayoutEngine, key: u64) -> [LayoutId; 3] {
    let plain = leaf(engine, 10.);
    let keyed = measured(engine, Some(key));
    [plain, keyed, parent(engine, &[plain, keyed])]
}

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
