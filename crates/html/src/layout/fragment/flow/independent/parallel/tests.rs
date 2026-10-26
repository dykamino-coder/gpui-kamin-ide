//! Normal конец и независимые продолжения при новой геометрии копии.
use super::*;
use crate::layout::fragment::flow::blocks::BlockKid;

fn block(id: u64, kids: Vec<BlockBox>) -> BlockBox {
    BlockBox {
        id,
        kids: kids
            .into_iter()
            .map(|b| BlockKid {
                id: b.id,
                inner: Some(b),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn normal_end_keeps_two_overflow_children_without_shifting_the_sibling() {
    let root = block(
        1,
        vec![
            block(2, vec![block(3, vec![]), block(4, vec![])]),
            block(5, vec![]),
        ],
    );
    let old = Laid::of_boxes(&[
        (1, 0., 70.),
        (2, 0., 40.),
        (3, 0., 160.),
        (4, 20., 200.),
        (5, 40., 70.),
    ]);
    let p = Parallel::at(&root, &old, 80., None).unwrap();
    let t = p.store.get(p.root.unwrap()).unwrap();
    assert!(t.at_block_end);
    let FcState::Block(s) = &t.fc else { panic!() };
    assert!(s.child.is_none());
    assert_eq!(s.parallel.len(), 1);
    let child = p.store.get(s.parallel[0].token).unwrap();
    assert!(child.at_block_end);
    let FcState::Block(s) = &child.fc else {
        panic!()
    };
    assert_eq!(s.parallel.iter().map(|c| c.idx).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(p.offset(&root, &old), Some(80.));
    assert_eq!(flow_end(&root, &old), Some(200.));
    assert!(
        Parallel::at(&root, &old, 200., None)
            .unwrap()
            .root
            .is_none()
    );
}

#[test]
fn resumed_leaf_maps_consumed_fraction_to_new_constraints() {
    let root = block(1, vec![block(2, vec![])]);
    let old = Laid::of_boxes(&[(1, 0., 40.), (2, 20., 180.)]);
    let new = Laid::of_boxes(&[(1, 0., 50.), (2, 30., 350.)]);
    let p = Parallel::at(&root, &old, 100., None).unwrap();
    assert_eq!(p.offset(&root, &new), Some(190.));
}

#[test]
fn clipping_finishes_the_flow_at_the_normal_end() {
    let mut root = block(1, vec![block(2, vec![])]);
    root.clips_block = true;
    let laid = Laid::of_boxes(&[(1, 0., 40.), (2, 0., 180.)]);
    assert_eq!(flow_end(&root, &laid), Some(40.));
    assert!(
        Parallel::at(&root, &laid, 40., None)
            .unwrap()
            .root
            .is_none()
    );
}

#[test]
fn geometric_order_does_not_hide_an_earlier_parallel_child() {
    let root = block(1, vec![block(2, vec![]), block(3, vec![])]);
    let laid = Laid::of_boxes(&[(1, 0., 40.), (2, 200., 240.), (3, 0., 180.)]);
    let p = Parallel::at(&root, &laid, 80., None).unwrap();
    assert_eq!(p.offset(&root, &laid), Some(80.));
}

#[test]
fn invalid_child_geometry_is_an_error() {
    let root = block(1, vec![block(2, vec![])]);
    let laid = Laid::of_boxes(&[(1, 0., 40.), (2, 100., 80.)]);
    assert!(matches!(
        Parallel::at(&root, &laid, 50., None),
        Err(Error::InvalidGeometry)
    ));
}

#[test]
fn consumed_content_and_sliced_edges_map_separately() {
    let mut child = block(2, vec![]);
    child.head = 10.;
    child.tail = 20.;
    let root = block(1, vec![child]);
    let old = Laid::of_boxes(&[(1, 0., 20.), (2, 0., 130.)]);
    let p = Parallel::at(&root, &old, 60., None).unwrap();
    let mut fresh = root.clone();
    fresh.kids[0].inner.as_mut().unwrap().head = 30.;
    fresh.kids[0].inner.as_mut().unwrap().tail = 40.;
    let new = Laid::of_boxes(&[(1, 0., 20.), (2, 0., 270.)]);
    assert_eq!(p.offset(&fresh, &new), Some(130.));
    let p = Parallel::at(&root, &old, 120., None).unwrap();
    assert_eq!(p.offset(&fresh, &new), Some(250.));
}

#[test]
fn parallel_sequence_advances_only_for_another_fragment() {
    let root = block(1, vec![block(2, vec![])]);
    let laid = Laid::of_boxes(&[(1, 0., 40.), (2, 0., 180.)]);
    let first = Parallel::at(&root, &laid, 60., None).unwrap();
    let next = Parallel::at(&root, &laid, 100., Some((&first, true))).unwrap();
    let replay = Parallel::at(&root, &laid, 100., Some((&next, false))).unwrap();
    assert_eq!(next.store.get(next.root.unwrap()).unwrap().seq, 2);
    assert_eq!(replay.store.get(replay.root.unwrap()).unwrap().seq, 2);
    assert_eq!(replay.store.get(replay.ids[&2]).unwrap().seq, 2);
    assert!((replay.offset(&root, &laid).unwrap() - 100.).abs() < COORD_EPS);
}

#[test]
fn future_normal_children_remain_unstarted_after_the_active_child() {
    let root = block(
        1,
        vec![block(2, vec![]), block(3, vec![]), block(4, vec![])],
    );
    let laid = Laid::of_boxes(&[(1, 0., 240.), (2, 0., 40.), (3, 40., 120.), (4, 120., 240.)]);
    let p = Parallel::at(&root, &laid, 80., None).unwrap();
    let token = p.store.get(p.root.unwrap()).unwrap();
    let FcState::Block(state) = &token.fc else {
        panic!()
    };
    assert!(!token.flags.seen_all_children);
    assert_eq!(state.child_idx, 1);
    assert!(state.child.is_some());
    assert!(state.parallel.is_empty());
    assert!(!p.ids.contains_key(&4));
    let children: Vec<_> = ChildCursor::new(3, state, false).unwrap().collect();
    assert_eq!(children.iter().map(|c| c.index).collect::<Vec<_>>(), [1, 2]);
    assert!(children[0].token.is_some());
    assert!(children[1].token.is_none());
}
