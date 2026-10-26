//! Последний fixed определяется после завершения abspos, независимо от конца normal flow.

use super::*;
use crate::layout::fragment::{flow::inline::Geometry, plan::oof::Flow, types::Kid};

#[test]
fn delayed_absolute_extends_plan_and_fixed_finishes_on_its_last_page() {
    let mut pending = vec![
        OofResume {
            idx: 0,
            block_start: 12.,
            inline_start: 0.,
            repeat_fixed: true,
            token: None,
        },
        OofResume {
            idx: 1,
            block_start: 155.,
            inline_start: 9.,
            repeat_fixed: false,
            token: None,
        },
    ];
    let mut flows = [Flow::default(), Flow::default()];
    let mut store = TokenStore::default();
    let mut actual = Vec::new();
    for (page, size) in [80., 100., 60., 50.].into_iter().enumerate() {
        let batch = resume_all(
            &pending,
            Space {
                block_size: Some(size),
                ..Space::pages()
            },
            true,
            &mut store,
            |idx, start, token, space, store| {
                flows[idx as usize].resume(
                    &Kid {
                        h: 120.,
                        over: 120.,
                        ..Default::default()
                    },
                    &Geometry::default(),
                    space,
                    start,
                    token,
                    store,
                )
            },
        )
        .unwrap();
        assert_eq!(batch.needs_space, page != 3);
        for (idx, fragment) in batch.fragments {
            actual.push((page, idx, fragment.y, fragment.from, fragment.h));
        }
        pending = batch.next;
    }
    assert!(pending.is_empty());
    assert_eq!(
        actual,
        [
            (0, 0, 12., 0., 120.),
            (1, 0, 12., 0., 120.),
            (1, 1, 75., 0., 25.),
            (2, 0, 12., 0., 120.),
            (2, 1, 0., 25., 60.),
            (3, 0, 12., 0., 120.),
            (3, 1, 0., 85., 35.),
        ]
    );
}

#[test]
fn fixed_alone_waits_for_normal_end_without_requesting_space() {
    let input = OofResume {
        idx: 0,
        block_start: 0.,
        inline_start: 0.,
        repeat_fixed: true,
        token: None,
    };
    let mut store = TokenStore::default();
    for normal_complete in [false, true] {
        let batch = resume_all(
            &[input],
            Space {
                block_size: Some(80.),
                ..Space::pages()
            },
            normal_complete,
            &mut store,
            |_, _, token, space, _| {
                assert!(token.is_none());
                assert_eq!(
                    space.repeat,
                    RepeatState::Repeat {
                        last: normal_complete
                    }
                );
                Ok(Child {
                    fragment: Some(()),
                    next: None,
                })
            },
        )
        .unwrap();
        assert!(!batch.needs_space);
        assert_eq!(batch.next.is_empty(), normal_complete);
    }
}
