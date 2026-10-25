//! Исполнение курсора: завершённые дети не повторяются, параллельные не теряются.
use super::super::cursor::ChildCursor;
use super::super::fc::{BlockState, ChildToken, FcState};
use super::super::{Error, Token, TokenId, TokenStore};

#[test]
fn resumes_pending_children_once_then_starts_unvisited_children() {
    let state = BlockState {
        child_idx: 4,
        child: Some(TokenId(40)),
        parallel: vec![
            ChildToken {
                idx: 1,
                token: TokenId(10),
            },
            ChildToken {
                idx: 3,
                token: TokenId(30),
            },
        ],
        ..Default::default()
    };
    let children: Vec<_> = ChildCursor::new(7, &state, false)
        .unwrap()
        .map(|c| (c.index, c.token))
        .collect();
    assert_eq!(
        children,
        vec![
            (1, Some(TokenId(10))),
            (3, Some(TokenId(30))),
            (4, Some(TokenId(40))),
            (5, None),
            (6, None)
        ]
    );
}

#[test]
fn seen_all_at_block_end_still_resumes_parallel_children() {
    let token = Token {
        flags: super::super::TokenFlags {
            seen_all_children: true,
            ..Default::default()
        },
        at_block_end: true,
        ..Token::new(FcState::Block(BlockState {
            child_idx: 3,
            parallel: vec![ChildToken {
                idx: 0,
                token: TokenId(4),
            }],
            ..Default::default()
        }))
    };
    let FcState::Block(state) = &token.fc else {
        unreachable!()
    };
    let children: Vec<_> = ChildCursor::new(3, state, token.flags.seen_all_children)
        .unwrap()
        .collect();
    assert_eq!(children.len(), 1);
    assert_eq!(
        (children[0].index, children[0].token),
        (0, Some(TokenId(4)))
    );
    assert!(ChildCursor::new(4, state, true).is_err());
}

#[test]
fn malformed_cursor_cannot_duplicate_or_skip_a_child() {
    for indices in [vec![1, 1], vec![2, 1], vec![4], vec![5]] {
        let state = BlockState {
            child_idx: 4,
            child: Some(TokenId(0)),
            parallel: indices
                .into_iter()
                .map(|idx| ChildToken {
                    idx,
                    token: TokenId(1),
                })
                .collect(),
            ..Default::default()
        };
        assert!(matches!(
            ChildCursor::new(7, &state, false),
            Err(Error::InvalidToken)
        ));
    }
    assert!(
        ChildCursor::new(
            2,
            &BlockState {
                child_idx: 3,
                ..Default::default()
            },
            false
        )
        .is_err()
    );
}

#[test]
fn sequence_exhaustion_and_invalid_geometry_are_explicit() {
    let fc = FcState::Block(BlockState::default());
    let mut token = Token::new(fc.clone());
    token.seq = u32::MAX;
    assert_eq!(
        token.advance(1.0, fc.clone()),
        Err(Error::ResourceExhausted)
    );
    token.seq = 1;
    for amount in [-1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            token.advance(amount, fc.clone()),
            Err(Error::InvalidGeometry)
        );
    }
    let mut store = TokenStore::default();
    let bad = Token::new(FcState::Block(BlockState {
        child: Some(TokenId(0)),
        ..Default::default()
    }));
    assert_eq!(store.push(bad), Err(Error::InvalidToken));
    assert_eq!(
        store.push(token),
        Ok(TokenId(0)),
        "failed insert consumed no ID"
    );
}
