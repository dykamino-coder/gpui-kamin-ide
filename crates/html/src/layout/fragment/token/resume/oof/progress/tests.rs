//! Продвижение OOF определяется содержимым, включая глубоко вложенные токены.

use super::*;
use crate::layout::fragment::token::fc::{BlockState, InlineState};

fn block() -> Token {
    Token::new(FcState::Block(BlockState::default()))
}

#[test]
fn metadata_and_backward_cursors_do_not_advance() {
    let store = TokenStore::default();
    let mut before = block();
    before.consumed = 40.;
    let mut after = before.clone();
    after.seq += 1;
    after.flags.seen_all_children = true;
    after.at_block_end = true;
    assert!(!advances(&before, &after, &store));
    after.consumed = 20.;
    let FcState::Block(state) = &mut after.fc else {
        unreachable!()
    };
    state.child_idx = 3;
    assert!(!advances(&before, &after, &store));
}

#[test]
fn actual_inline_cursor_advances_without_block_consumption() {
    let store = TokenStore::default();
    let before = Token::new(FcState::Inline(InlineState::default()));
    let mut after = before.clone();
    let FcState::Inline(state) = &mut after.fc else {
        unreachable!()
    };
    state.text_pos = 12;
    assert!(advances(&before, &after, &store));
    assert!(!advances(&after, &before, &store));
}

#[test]
fn parallel_children_match_identity_after_an_earlier_child_finishes() {
    let mut store = TokenStore::default();
    let child = |consumed| {
        let mut t = block();
        t.consumed = consumed;
        t
    };
    let first = store.push(child(1.)).unwrap();
    let old_second = store.push(child(40.)).unwrap();
    let new_second = store.push(child(40.)).unwrap();
    let parent = |parallel| {
        Token::new(FcState::Block(BlockState {
            child_idx: 2,
            parallel,
            ..Default::default()
        }))
    };
    let before = parent(vec![
        ChildToken {
            idx: 0,
            token: first,
        },
        ChildToken {
            idx: 1,
            token: old_second,
        },
    ]);
    let after = parent(vec![ChildToken {
        idx: 1,
        token: new_second,
    }]);
    assert_eq!(
        matching_children(&before.fc, &after.fc),
        (vec![(old_second, new_second)], true)
    );
    assert!(advances(&before, &after, &store));
    let advanced = store.push(child(41.)).unwrap();
    let after = parent(vec![ChildToken {
        idx: 1,
        token: advanced,
    }]);
    assert!(advances(&before, &after, &store));
}

#[test]
fn nested_oof_progress_includes_delayed_start_and_completion() {
    use crate::layout::fragment::token::resume::OofResume;
    let store = TokenStore::default();
    let mut before = block();
    before.at_block_end = true;
    before.oof.push(OofResume {
        idx: 2,
        block_start: 150.,
        inline_start: 9.,
        repeat_fixed: false,
        token: None,
    });
    let mut after = before.clone();
    assert!(!advances(&before, &after, &store));
    after.oof[0].block_start = 50.;
    assert!(advances(&before, &after, &store));
    after.oof.clear();
    assert!(advances(&before, &after, &store));
    before.oof[0].repeat_fixed = true;
    assert!(!advances(&before, &after, &store));
}

#[test]
fn sliced_decoration_advances_without_consuming_content() {
    let store = TokenStore::default();
    let before = block();
    let after = Token::new(FcState::Block(BlockState {
        sliced_edges: Some((0.5, 0.)),
        ..Default::default()
    }));
    assert!(advances(&before, &after, &store));
    assert!(!advances(&after, &before, &store));
}

#[test]
fn nested_child_progress_uses_heap_worklist() {
    let mut store = TokenStore::default();
    let before = block();
    let mut after = block();
    after.consumed = 1.;
    let mut pair = (store.push(before).unwrap(), store.push(after).unwrap());
    for _ in 0..10_000 {
        let wrap = |child| {
            Token::new(FcState::Block(BlockState {
                child: Some(child),
                ..Default::default()
            }))
        };
        pair = (
            store.push(wrap(pair.0)).unwrap(),
            store.push(wrap(pair.1)).unwrap(),
        );
    }
    assert!(advances(
        store.get(pair.0).unwrap(),
        store.get(pair.1).unwrap(),
        &store
    ));
    assert!(!advances(
        store.get(pair.1).unwrap(),
        store.get(pair.0).unwrap(),
        &store
    ));
}
