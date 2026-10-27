//! Независимость OOF, переменные размеры и последний повтор печатного fixed.

use super::*;
use crate::layout::fragment::token::{
    Token,
    fc::{BlockState, FcState},
};

fn pending(start: f32, policy: FixedPolicy) -> OofResume {
    OofResume {
        idx: 7,
        block_start: start,
        inline_start: 23.,
        repeat_fixed: policy == FixedPolicy::RepeatPages,
        token: None,
    }
}

#[test]
fn delayed_start_consumes_each_actual_fragmentainer_without_starting_child() {
    let mut store = TokenStore::default();
    let mut state = pending(250., FixedPolicy::Fragment);
    for block in [80., 130.] {
        let output = resume::<()>(
            state,
            Space {
                block_size: Some(block),
                ..Space::pages()
            },
            &mut store,
            |_, _, _| panic!("ребёнок ещё не начат"),
        )
        .unwrap();
        assert!(output.fragment.is_none());
        assert!(output.needs_space());
        state = output.next.unwrap();
    }
    let output = resume(
        state,
        Space {
            block_size: Some(60.),
            ..Space::pages()
        },
        &mut store,
        |start, token, _| {
            assert_eq!(start, 40.);
            assert!(token.is_none());
            Ok(Child {
                fragment: Some(start),
                next: None,
            })
        },
    )
    .unwrap();
    assert_eq!(output.fragment, Some(40.));
    assert!(output.next.is_none());
}

#[test]
fn child_token_resumes_independently_after_normal_block_end() {
    let mut store = TokenStore::default();
    let mut state = pending(15., FixedPolicy::Fragment);
    for (i, block) in [90., 70., 45.].into_iter().enumerate() {
        let output = resume(
            state,
            Space {
                block_size: Some(block),
                ..Space::pages()
            },
            &mut store,
            |start, token, store| {
                let consumed = token.map_or(0., |id| store.get(id).unwrap().consumed);
                assert_eq!(start, if i == 0 { 15. } else { 0. });
                let end = consumed + block - start;
                let next = if i == 2 {
                    None
                } else {
                    let mut next = Token::new(FcState::Block(BlockState::default()));
                    next.consumed = end;
                    next.at_block_end = true;
                    Some(store.push(next)?)
                };
                Ok(Child {
                    fragment: Some((consumed, end)),
                    next,
                })
            },
        )
        .unwrap();
        if i == 2 {
            assert!(output.next.is_none());
        } else {
            state = output.next.unwrap();
        }
    }
}

#[test]
fn fixed_repeat_does_not_request_pages_and_last_repeat_completes() {
    let mut store = TokenStore::default();
    let state = pending(12., FixedPolicy::for_box(true, false));
    for last in [false, true] {
        let output = resume(
            state,
            Space {
                block_size: Some(100.),
                repeat: RepeatState::Repeat { last },
                ..Space::pages()
            },
            &mut store,
            |start, token, _| {
                assert_eq!(start, 12.);
                assert!(token.is_none());
                Ok(Child {
                    fragment: Some(()),
                    next: None,
                })
            },
        )
        .unwrap();
        assert!(!output.needs_space());
        assert_eq!(output.next.is_none(), last);
    }
    assert_eq!(FixedPolicy::for_box(true, true), FixedPolicy::Fragment);
    assert_eq!(FixedPolicy::for_box(false, true), FixedPolicy::Fragment);
}

#[test]
fn invalid_space_and_stalled_child_are_reported() {
    let mut store = TokenStore::default();
    let token = store
        .push(Token::new(FcState::Block(BlockState::default())))
        .unwrap();
    let mut state = pending(0., FixedPolicy::Fragment);
    state.token = Some(token);
    let output = resume::<()>(
        state,
        Space {
            block_size: Some(100.),
            ..Space::pages()
        },
        &mut store,
        |_, _, _| {
            Ok(Child {
                fragment: None,
                next: Some(token),
            })
        },
    );
    assert!(matches!(output, Err(Error::NoProgress)));
    assert!(matches!(
        resume::<()>(
            state,
            Space {
                block_size: Some(0.),
                ..Space::pages()
            },
            &mut store,
            |_, _, _| unreachable!()
        ),
        Err(Error::InvalidGeometry)
    ));
}

#[test]
fn break_before_cannot_repeat_on_an_empty_fragmentainer() {
    let mut store = TokenStore::default();
    let mut token = Token::new(FcState::Block(BlockState::default()));
    token.flags.break_before = true;
    let token = store.push(token).unwrap();
    let state = OofResume {
        token: Some(token),
        ..pending(0., FixedPolicy::Fragment)
    };
    for not_at_start in [1, 0] {
        let output = resume::<()>(
            state,
            Space {
                block_size: Some(100.),
                not_at_start,
                ..Space::pages()
            },
            &mut store,
            |_, _, _| {
                Ok(Child {
                    fragment: None,
                    next: Some(token),
                })
            },
        );
        if not_at_start == 1 {
            assert!(output.is_ok());
        } else {
            assert!(matches!(output, Err(Error::NoProgress)));
        }
    }
}

#[test]
fn page_repeat_requires_an_explicit_final_repeat_contract() {
    let mut store = TokenStore::default();
    let state = pending(0., FixedPolicy::RepeatPages);
    for space in [Space::columns(), Space::pages()] {
        let result = resume::<()>(
            state,
            Space {
                block_size: Some(100.),
                ..space
            },
            &mut store,
            |_, _, _| unreachable!(),
        );
        assert!(matches!(result, Err(Error::InvalidGeometry)));
    }
}

#[test]
fn delayed_start_must_advance_even_at_float_precision_limit() {
    let mut store = TokenStore::default();
    let output = resume::<()>(
        pending(f32::MAX, FixedPolicy::Fragment),
        Space {
            block_size: Some(1.),
            ..Space::pages()
        },
        &mut store,
        |_, _, _| unreachable!(),
    );
    assert!(matches!(output, Err(Error::NoProgress)));
}
