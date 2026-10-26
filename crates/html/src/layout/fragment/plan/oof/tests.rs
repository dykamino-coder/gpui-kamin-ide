//! Независимый OOF использует те же разрывы, что и основной планировщик.

use super::*;
use crate::layout::fragment::token::resume::{OofResume, oof};
use crate::layout::fragment::token::space::RepeatState;

fn slice(h: f32) -> Kid {
    Kid {
        h,
        over: h,
        ..Default::default()
    }
}

#[test]
fn independent_steps_match_normal_plan_with_forced_breaks() {
    let kid = Kid {
        forced: vec![35., 120.],
        ..slice(185.)
    };
    let (_, _, expected) = crate::layout::fragment::plan::fill(
        std::slice::from_ref(&kid),
        70.,
        super::super::Space::pages(10),
    );
    let mut flow = Flow::default();
    let mut store = TokenStore::default();
    let mut token = None;
    for expected in expected.iter() {
        let result = flow
            .resume(
                &kid,
                &Geometry::default(),
                Space {
                    block_size: Some(70.),
                    ..Space::pages()
                },
                0.,
                token,
                &mut store,
            )
            .unwrap();
        let actual = result.fragment.unwrap();
        assert_eq!(
            (actual.copy, actual.from, actual.h, actual.head, actual.foot),
            (
                expected.copy,
                expected.from,
                expected.h,
                expected.head,
                expected.foot
            )
        );
        token = result.next;
    }
    assert!(token.is_none());
}

#[test]
fn delayed_oof_resumes_through_different_actual_page_sizes() {
    let kid = slice(120.);
    let mut flow = Flow::default();
    let mut store = TokenStore::default();
    let mut input = OofResume {
        idx: 0,
        block_start: 155.,
        inline_start: 9.,
        repeat_fixed: false,
        token: None,
    };
    let mut fragments = Vec::new();
    for size in [80., 100., 60., 50.] {
        let space = Space {
            block_size: Some(size),
            ..Space::pages()
        };
        let result = oof::resume(input, space, &mut store, |start, token, store| {
            flow.resume(&kid, &Geometry::default(), space, start, token, store)
        })
        .unwrap();
        if let Some(fragment) = result.fragment {
            fragments.push((fragment.y, fragment.from, fragment.h));
        }
        match result.next {
            Some(next) => input = next,
            None => break,
        }
    }
    assert_eq!(fragments, [(75., 0., 25.), (0., 25., 60.), (0., 85., 35.)]);
}

#[test]
fn monolith_moves_once_then_overflows_empty_column() {
    let kid = Kid {
        monolith: true,
        ..slice(150.)
    };
    let mut flow = Flow::default();
    let mut store = TokenStore::default();
    let space = Space {
        block_size: Some(100.),
        ..Space::columns()
    };
    let moved = flow
        .resume(&kid, &Geometry::default(), space, 20., None, &mut store)
        .unwrap();
    assert!(moved.fragment.is_none());
    let result = flow
        .resume(
            &kid,
            &Geometry::default(),
            space,
            0.,
            moved.next,
            &mut store,
        )
        .unwrap();
    assert_eq!(result.fragment.unwrap().h, 150.);
    assert!(result.next.is_none());
}

#[test]
fn fixed_repeat_keeps_full_geometry_on_last_page() {
    let kid = slice(185.);
    let mut flow = Flow::default();
    let mut store = TokenStore::default();
    for last in [false, true] {
        let space = Space {
            block_size: Some(70.),
            repeat: RepeatState::Repeat { last },
            ..Space::pages()
        };
        let result = flow
            .resume(&kid, &Geometry::default(), space, 12., None, &mut store)
            .unwrap();
        let fragment = result.fragment.unwrap();
        assert_eq!((fragment.y, fragment.from, fragment.h), (12., 0., 185.));
        assert!(result.next.is_none());
    }
}

#[test]
fn absolute_inside_a_repeated_root_is_not_delayed_or_fragmented() {
    let kid = slice(185.);
    let mut flow = Flow::default();
    let mut store = TokenStore::default();
    let space = Space {
        block_size: Some(70.),
        repeat: RepeatState::Repeat { last: true },
        ..Space::pages()
    };
    let result = oof::resume(
        OofResume {
            idx: 0,
            block_start: 150.,
            inline_start: 0.,
            repeat_fixed: false,
            token: None,
        },
        space,
        &mut store,
        |start, token, store| flow.resume(&kid, &Geometry::default(), space, start, token, store),
    )
    .unwrap();
    let fragment = result.fragment.unwrap();
    assert_eq!((fragment.y, fragment.from, fragment.h), (150., 0., 185.));
    assert!(result.next.is_none());
}
