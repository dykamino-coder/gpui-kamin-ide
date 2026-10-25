//! Проверки реального resume: потребление, отложенное начало и явный отказ.
use super::*;

fn space(block: f32) -> Space {
    Space {
        block_size: Some(block),
        ..Space::pages()
    }
}

#[test]
fn remaining_box_uses_each_fragmentainers_constraint() {
    let leaf = Leaf {
        content_block: 250.0,
        ..Leaf::default()
    };
    let mut store = TokenStore::default();
    let first = leaf.resume(space(100.0), 30.0, None, &mut store).unwrap();
    assert_eq!(
        first.fragment,
        Some(Fragment {
            from: 0.0,
            block_size: 70.0
        })
    );
    let t = store.get(first.next.unwrap()).unwrap();
    assert_eq!((t.consumed, t.seq, t.at_block_end), (70.0, 1, false));
    let second = leaf
        .resume(space(120.0), 0.0, first.next, &mut store)
        .unwrap();
    assert_eq!(
        second.fragment,
        Some(Fragment {
            from: 70.0,
            block_size: 120.0
        })
    );
    let t = store.get(second.next.unwrap()).unwrap();
    assert_eq!((t.consumed, t.seq), (190.0, 2));
    let last = leaf
        .resume(space(90.0), 0.0, second.next, &mut store)
        .unwrap();
    assert_eq!(
        last.fragment,
        Some(Fragment {
            from: 190.0,
            block_size: 60.0
        })
    );
    assert_eq!(last.next, None);
}

#[test]
fn break_before_does_not_create_a_fragment_or_consume_space() {
    let leaf = Leaf {
        content_block: 150.0,
        ..Leaf::default()
    };
    let mut store = TokenStore::default();
    let before = leaf.resume(space(100.0), 100.0, None, &mut store).unwrap();
    assert_eq!(before.fragment, None);
    let t = store.get(before.next.unwrap()).unwrap();
    assert!(t.flags.break_before);
    assert_eq!((t.consumed, t.seq), (0.0, 0));
    let next = leaf
        .resume(space(100.0), 0.0, before.next, &mut store)
        .unwrap();
    let t = store.get(next.next.unwrap()).unwrap();
    assert!(!t.flags.break_before);
    assert_eq!((t.consumed, t.seq), (100.0, 1));
}

#[test]
fn exhausted_empty_fragmentainer_is_an_error_and_empty_box_completes() {
    let mut store = TokenStore::default();
    assert_eq!(
        Leaf {
            content_block: 1.0,
            ..Leaf::default()
        }
        .resume(space(0.0), 0.0, None, &mut store),
        Err(Error::NoProgress)
    );
    let empty = Leaf {
        content_block: 0.0,
        ..Leaf::default()
    }
    .resume(space(0.0), 0.0, None, &mut store)
    .unwrap();
    assert_eq!(
        empty,
        Output {
            fragment: Some(Fragment {
                from: 0.0,
                block_size: 0.0
            }),
            next: None
        }
    );
}

#[test]
fn negative_placement_is_not_negative_consumption() {
    let mut store = TokenStore::default();
    let out = Leaf {
        content_block: 120.0,
        ..Leaf::default()
    }
    .resume(space(100.0), -20.0, None, &mut store)
    .unwrap();
    assert_eq!(out.fragment.unwrap().block_size, 120.0);
    assert_eq!(out.next, None);
}

#[test]
fn impossible_geometry_and_foreign_context_are_rejected() {
    let mut store = TokenStore::default();
    for size in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            Leaf {
                content_block: size,
                ..Leaf::default()
            }
            .resume(space(100.0), 0.0, None, &mut store),
            Err(Error::InvalidGeometry)
        );
    }
    let id = store
        .push(Token::new(FcState::Inline(Default::default())))
        .unwrap();
    assert_eq!(
        Leaf {
            content_block: 10.0,
            ..Leaf::default()
        }
        .resume(space(100.0), 0.0, Some(id), &mut store),
        Err(Error::InvalidToken)
    );
}

#[test]
fn rounding_cannot_turn_a_resume_into_an_infinite_loop() {
    let mut token = Token::new(FcState::Block(BlockState::default()));
    token.consumed = 16_777_216.0;
    let mut store = TokenStore::default();
    let id = store.push(token).unwrap();
    assert_eq!(
        Leaf {
            content_block: 33_554_432.0,
            ..Leaf::default()
        }
        .resume(space(0.5), 0.0, Some(id), &mut store),
        Err(Error::NoProgress)
    );
}
