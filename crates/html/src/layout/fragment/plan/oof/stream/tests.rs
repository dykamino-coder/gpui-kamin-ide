//! Очередь использует фактические размеры, независимые якоря и последний repeat.

use super::*;

fn input(first: usize, start: f32, h: f32, fixed: bool) -> Input {
    Input {
        kid: Kid {
            h,
            over: h,
            ..Default::default()
        },
        inline: Geometry::default(),
        first,
        block_start: start,
        inline_start: 7.,
        repeat_fixed: fixed,
        fixed_content: fixed,
    }
}

fn page(h: f32) -> Space {
    Space {
        block_size: Some(h),
        ..Space::pages()
    }
}

#[test]
fn delayed_stream_and_fixed_finish_on_the_actual_last_page() {
    let mut streams =
        Streams::new(vec![input(1, 95., 85., false), input(0, 5., 10., true)]).unwrap();
    let mut abs = Vec::new();
    for (index, h) in [70., 60., 80., 50.].into_iter().enumerate() {
        let batch = streams.resume(index, page(h), true).unwrap();
        assert!(batch.fragments.iter().any(|(id, f)| *id == 1 && f.y == 5.));
        abs.extend(
            batch
                .fragments
                .into_iter()
                .filter(|(id, _)| *id == 0)
                .map(|(_, f)| (f.col, f.y, f.from, f.h)),
        );
        assert_eq!(batch.next.iter().any(|p| p.repeat_fixed), index < 3);
        assert_eq!(batch.needs_space, index < 3);
    }
    assert_eq!(abs, [(2, 35., 0., 45.), (3, 0., 45., 40.)]);
    assert!(!streams.needs_space());
}

#[test]
fn independent_streams_do_not_move_each_others_start() {
    let mut streams =
        Streams::new(vec![input(0, 20., 100., false), input(0, 5., 12., false)]).unwrap();
    let batch = streams.resume(0, page(60.), true).unwrap();
    assert_eq!(
        batch
            .fragments
            .iter()
            .map(|(id, f)| (*id, f.y, f.h))
            .collect::<Vec<_>>(),
        [(0, 20., 40.), (1, 5., 12.)]
    );
    assert_eq!(batch.next.len(), 1);
    let batch = streams.resume(1, page(80.), true).unwrap();
    assert_eq!(batch.fragments.len(), 1);
    assert_eq!(batch.fragments[0].1.h, 60.);
    assert!(!batch.needs_space);
}

#[test]
fn skipping_a_fragmentainer_is_an_invalid_resume() {
    let mut streams = Streams::new(vec![input(0, 0., 100., false)]).unwrap();
    assert!(matches!(
        streams.resume(1, page(60.), true),
        Err(Error::InvalidToken)
    ));
    assert!(streams.resume(0, page(60.), true).is_ok());
}

#[test]
fn fixed_in_a_transformed_cb_overflows_without_paginating_or_repeating() {
    let mut contained = input(0, 95., 150., false);
    contained.fixed_content = true;
    let mut streams = Streams::new(vec![contained]).unwrap();
    let batch = streams.resume(0, page(60.), true).unwrap();
    assert_eq!(batch.fragments.len(), 1);
    let f = &batch.fragments[0].1;
    assert_eq!((f.col, f.y, f.from, f.h), (0, 95., 0., 150.));
    assert!(batch.next.is_empty());
    assert!(!batch.needs_space);
}

#[test]
fn fresh_oof_trials_report_shortage_and_do_not_share_tokens() {
    for height in [50., 100., 50.] {
        let mut positioned = input(0, 20., 80., false);
        positioned.kid.monolith = true;
        let mut streams = Streams::new(vec![positioned]).unwrap();
        let batch = streams
            .resume(
                0,
                Space {
                    block_size: Some(height),
                    ..Space::columns()
                },
                true,
            )
            .unwrap();
        if height == 50. {
            assert!(batch.fragments.is_empty());
            assert!(batch.needs_space);
            assert_eq!(streams.shortage(), Some(50.));
        } else {
            assert_eq!(batch.fragments[0].1.h, 80.);
            assert!(!batch.needs_space);
        }
    }
}
