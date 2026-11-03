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
