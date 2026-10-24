//! Токены: продолжение по контекстам форматирования, бюджет строк, хранилище вложенных токенов.

use crate::layout::fragment::token::fc::{
    BlockState, ChildToken, FcState, FlexState, FloatExclusion, FloatSide, GridState, InlineState,
    MulticolState, TablePhase, TableState,
};
use crate::layout::fragment::token::resume::{ClampBudget, OofResume};
use crate::layout::fragment::token::{Token, TokenFlags, TokenId, TokenStore};

#[test]
fn advance_accumulates_consumed_size_and_keeps_shared_state() {
    let block = |child_idx| {
        FcState::Block(BlockState {
            child_idx,
            ..Default::default()
        })
    };
    let mut first = Token::new(block(1));
    first.clamp = Some(ClampBudget {
        lines_left: 2,
        discard: true,
    });
    first.at_block_end = true;
    first.monolithic_overflow = 12.0;
    first.flags = TokenFlags {
        break_before: true,
        forced: true,
        repeated: true,
        by_spanner: true,
        seen_all_children: true,
    };
    let next = first
        .advance(80.0, block(3))
        .unwrap()
        .advance(0.0, block(4))
        .unwrap();
    assert_eq!((next.consumed, next.seq), (80.0, 3));
    assert_eq!(next.clamp, first.clamp);
    assert!(!next.at_block_end && next.monolithic_overflow == 0.0);
    assert_eq!(next.flags, TokenFlags::default());
    assert!(matches!(next.fc, FcState::Block(ref b) if b.child_idx == 4));
}

#[test]
fn clamp_budget_runs_across_fragmentainers() {
    let mut budget = ClampBudget {
        lines_left: 3,
        discard: false,
    };
    assert_eq!(budget.take(2), (2, false));
    assert_eq!(budget.take(5), (1, true));
    assert_eq!((budget.take(1), budget.discard), ((0, true), false));
}

#[test]
fn store_walks_nested_tokens_parent_first() {
    let mut store = TokenStore::default();
    let leaf = |store: &mut TokenStore, line_idx| {
        store
            .push(Token::new(FcState::Inline(InlineState {
                line_idx,
                text_pos: 7,
                past_first_line: true,
                child: None,
            })))
            .unwrap()
    };
    let (a, b, c) = (
        leaf(&mut store, 1),
        leaf(&mut store, 2),
        leaf(&mut store, 3),
    );
    let mut root = Token::new(FcState::Block(BlockState {
        child_idx: 0,
        margin_strut: (8.0, -2.0),
        float_exclusions: vec![FloatExclusion {
            side: FloatSide::InlineEnd,
            inline_size: 40.0,
            block_end: 25.0,
        }],
        child: Some(a),
        parallel: vec![ChildToken { idx: 2, token: b }],
    }));
    root.oof.push(OofResume {
        idx: 0,
        block_start: 300.0,
        inline_start: 12.0,
        repeat_fixed: false,
        token: Some(c),
    });
    let r = store.push(root).unwrap();
    assert_eq!(store.subtree(r), vec![r, a, b, c]);
    let Some(FcState::Block(state)) = store.get(r).map(|t| &t.fc) else {
        unreachable!("pushed a block token");
    };
    let float = state.float_exclusions[0];
    assert_eq!(
        (float.side, float.inline_size, float.block_end),
        (FloatSide::InlineEnd, 40.0, 25.0)
    );
    assert_eq!(
        (state.margin_strut, state.parallel[0].idx),
        ((8.0, -2.0), 2)
    );
    let oof = store.get(r).map(|t| t.oof[0]).expect("pushed");
    assert_eq!(
        (oof.idx, oof.block_start, oof.inline_start, oof.repeat_fixed),
        (0, 300.0, 12.0, false)
    );
    let Some(FcState::Inline(line)) = store.get(a).map(|t| &t.fc) else {
        unreachable!("pushed an inline token");
    };
    assert_eq!(
        (line.line_idx, line.text_pos, line.past_first_line),
        (1, 7, true)
    );
    assert_eq!(store.get(TokenId(99)), None);
}

#[test]
fn every_context_reports_its_child_tokens() {
    let mut store = TokenStore::default();
    let t = store
        .push(Token::new(FcState::Block(BlockState::default())))
        .unwrap();
    let kids = vec![ChildToken { idx: 1, token: t }];
    let flex = FlexState {
        line: 2,
        items: kids.clone(),
        offset_adjust: 4.0,
    };
    let grid = GridState {
        row_track: 3,
        items: kids.clone(),
        offset_adjust: 6.0,
    };
    let table = TableState {
        phase: TablePhase::InBox,
        section: 1,
        row: 5,
        repeat_header: true,
        repeat_footer: false,
        cells: kids,
        consumed_box: 90.0,
    };
    let multicol = MulticolState {
        row: 1,
        spanner: Some(0),
        flow: Some(t),
    };
    assert_eq!(
        (
            flex.line,
            flex.offset_adjust,
            grid.row_track,
            grid.offset_adjust
        ),
        (2, 4.0, 3, 6.0)
    );
    assert_eq!((table.section, table.row, table.consumed_box), (1, 5, 90.0));
    assert!(table.repeat_header && !table.repeat_footer);
    assert_eq!((multicol.row, multicol.spanner), (1, Some(0)));
    assert_ne!(TablePhase::default(), TablePhase::PastBox);
    for fc in [
        FcState::Flex(flex),
        FcState::Grid(grid),
        FcState::Table(table),
        FcState::Multicol(multicol),
        FcState::Inline(InlineState {
            child: Some(t),
            ..Default::default()
        }),
    ] {
        assert_eq!(fc.child_tokens().collect::<Vec<_>>(), vec![t]);
    }
    assert_eq!(
        FcState::Block(BlockState::default())
            .child_tokens()
            .collect::<Vec<_>>(),
        vec![]
    );
    assert_ne!(FloatSide::InlineStart, FloatSide::InlineEnd);
}
