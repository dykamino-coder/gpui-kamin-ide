//! Расход содержимого не включает края slice/clone и незаполненный хвост листа.
use super::*;

fn space(block: f32) -> Space {
    Space {
        block_size: Some(block),
        ..Space::pages()
    }
}

#[test]
fn slice_consumes_content_and_emits_each_edge_once() {
    let leaf = Leaf {
        content_block: 160.0,
        edges: Edges {
            start: 10.0,
            end: 20.0,
            clone: false,
        },
    };
    let mut store = TokenStore::default();
    let first = leaf.resume(space(100.0), 20.0, None, &mut store).unwrap();
    assert_eq!(
        first.fragment,
        Some(Fragment {
            from: 0.0,
            block_size: 80.0
        })
    );
    assert_eq!(store.get(first.next.unwrap()).unwrap().consumed, 70.0);
    let second = leaf
        .resume(space(120.0), 0.0, first.next, &mut store)
        .unwrap();
    assert_eq!(
        second.fragment,
        Some(Fragment {
            from: 80.0,
            block_size: 110.0
        })
    );
    assert_eq!(second.next, None);
}

#[test]
fn clone_reserves_both_edges_without_consuming_them() {
    let leaf = Leaf {
        content_block: 160.0,
        edges: Edges {
            start: 10.0,
            end: 20.0,
            clone: true,
        },
    };
    let mut store = TokenStore::default();
    let first = leaf.resume(space(100.0), 20.0, None, &mut store).unwrap();
    assert_eq!(
        first.fragment,
        Some(Fragment {
            from: 0.0,
            block_size: 80.0
        })
    );
    assert_eq!(store.get(first.next.unwrap()).unwrap().consumed, 50.0);
    let second = leaf
        .resume(space(120.0), 0.0, first.next, &mut store)
        .unwrap();
    assert_eq!(
        second.fragment,
        Some(Fragment {
            from: 50.0,
            block_size: 120.0
        })
    );
    assert_eq!(store.get(second.next.unwrap()).unwrap().consumed, 140.0);
    let last = leaf
        .resume(space(80.0), 0.0, second.next, &mut store)
        .unwrap();
    assert_eq!(
        last.fragment,
        Some(Fragment {
            from: 140.0,
            block_size: 50.0
        })
    );
    assert_eq!(last.next, None);
}

#[test]
fn start_edge_can_fill_a_slice_before_content_begins() {
    let leaf = Leaf {
        content_block: 30.0,
        edges: Edges {
            start: 20.0,
            end: 10.0,
            clone: false,
        },
    };
    let mut store = TokenStore::default();
    let first = leaf.resume(space(100.0), 80.0, None, &mut store).unwrap();
    let token = store.get(first.next.unwrap()).unwrap();
    assert_eq!(token.consumed, 0.0);
    assert!(!token.flags.break_before);
    let last = leaf
        .resume(space(100.0), 0.0, first.next, &mut store)
        .unwrap();
    assert_eq!(
        last.fragment,
        Some(Fragment {
            from: 20.0,
            block_size: 40.0
        })
    );
    assert_eq!(last.next, None);
}

#[test]
fn changed_page_size_preserves_the_consumed_fraction() {
    let mut store = TokenStore::default();
    let first = Leaf {
        content_block: 200.0,
        ..Leaf::default()
    }
    .resume(space(100.0), 0.0, None, &mut store)
    .unwrap();
    let last = Leaf {
        content_block: 400.0,
        ..Leaf::default()
    }
    .resume(space(200.0), 0.0, first.next, &mut store)
    .unwrap();
    assert_eq!(
        last.fragment,
        Some(Fragment {
            from: 200.0,
            block_size: 200.0
        })
    );
    assert_eq!(last.next, None);
}

#[test]
fn oversized_clone_edges_are_truncated_end_then_start() {
    let leaf = Leaf {
        content_block: 2.0,
        edges: Edges {
            start: 10.0,
            end: 20.0,
            clone: true,
        },
    };
    let mut store = TokenStore::default();
    let first = leaf.resume(space(5.0), 0.0, None, &mut store).unwrap();
    assert_eq!(first.fragment.unwrap().block_size, 5.0);
    assert_eq!(store.get(first.next.unwrap()).unwrap().consumed, 1.0);
    let last = leaf
        .resume(space(5.0), 0.0, first.next, &mut store)
        .unwrap();
    assert_eq!(last.fragment.unwrap().block_size, 5.0);
    assert_eq!(last.next, None);
}

#[test]
fn clone_defers_before_truncating_and_keeps_fractional_content_room() {
    let leaf = Leaf {
        content_block: 10.0,
        edges: Edges {
            start: 5.0,
            end: 5.0,
            clone: true,
        },
    };
    let mut store = TokenStore::default();
    let deferred = leaf.resume(space(100.0), 95.0, None, &mut store).unwrap();
    assert_eq!(deferred.fragment, None);
    let whole = leaf
        .resume(space(100.0), 0.0, deferred.next, &mut store)
        .unwrap();
    assert_eq!(whole.fragment.unwrap().block_size, 20.0);
    assert_eq!(whole.next, None);
    let fractional = leaf.resume(space(10.5), 0.0, None, &mut store).unwrap();
    assert_eq!(fractional.fragment.unwrap().block_size, 10.5);
    assert_eq!(store.get(fractional.next.unwrap()).unwrap().consumed, 0.5);
}

#[test]
fn slice_edges_span_pages_without_advancing_content() {
    let leaf = Leaf {
        content_block: 2.0,
        edges: Edges {
            start: 12.0,
            end: 11.0,
            clone: false,
        },
    };
    let mut store = TokenStore::default();
    let mut next = None;
    let mut heights = Vec::new();
    let mut consumed = Vec::new();
    loop {
        let output = leaf.resume(space(5.0), 0.0, next, &mut store).unwrap();
        heights.push(output.fragment.unwrap().block_size);
        next = output.next;
        if let Some(id) = next {
            consumed.push(store.get(id).unwrap().consumed);
        } else {
            break;
        }
    }
    assert_eq!(heights, vec![5.0; 5]);
    assert_eq!(consumed, vec![0.0, 0.0, 2.0, 2.0]);
}
