//! Пространство фрагментаинера: вид контекста, вложенные ряды, повтор, личность страницы.

use crate::layout::fragment::token::Space;
use crate::layout::fragment::token::space::{
    BreakAppeal, Kind, NestedRow, PageIdentity, PageSide, RepeatState,
};
use crate::layout::fragment::types::StackAxis;
use crate::style::computed::writing_mode::WritingMode;

fn wm(vertical: bool, rl: bool, rtl: bool) -> WritingMode {
    WritingMode::new(vertical, rl, false, rtl)
}

#[test]
fn first_page_side_follows_page_progression() {
    let side = |w: WritingMode| (PageSide::of(0, w), PageSide::of(1, w));
    assert_eq!(
        side(wm(false, false, false)),
        (PageSide::Right, PageSide::Left)
    );
    assert_eq!(
        side(wm(false, false, true)),
        (PageSide::Left, PageSide::Right)
    );
    assert_eq!(side(wm(true, true, false)).0, PageSide::Left);
    assert_eq!(side(wm(true, false, true)).0, PageSide::Right);
}

#[test]
fn page_identity_marks_the_first_page() {
    let page = |index| PageIdentity {
        index,
        name: Some(2),
        side: PageSide::of(index, WritingMode::default()),
        blank: index == 1,
    };
    let (first, second) = (page(0), page(1));
    assert!(first.is_first() && !second.is_first());
    assert_eq!(
        (second.name, second.side, second.blank),
        (Some(2), PageSide::Left, true)
    );
    let space = Space {
        page: Some(first),
        ..Space::pages()
    };
    assert_eq!(space.page.map(|p| p.index), Some(0));
}

#[test]
fn nested_row_first_row_takes_the_rest_of_the_outer_fragmentainer() {
    let row = NestedRow::from_outer_row((100.0, 30.0), false);
    assert_eq!((row.row_block(0), row.row_block(1)), (70.0, 100.0));
    let top = NestedRow::from_outer_row((100.0, -5.0), true);
    assert_eq!(
        (top.offset, top.outer_block, top.outer_paged),
        (0.0, 100.0, true)
    );
    assert_eq!(top.row_block(0), 100.0);
}

#[test]
fn nesting_starts_a_row_below_the_outer_start() {
    let page = Space {
        block_size: Some(200.0),
        inline_size: Some(300.0),
        ..Space::pages()
    };
    let inner = page.nest_at(50.0).expect("known outer block size");
    assert!(matches!(inner.kind, Kind::NestedRow(r) if r.outer_paged && r.offset == 50.0));
    assert_eq!(
        (inner.block_size, inner.inline_size),
        (Some(150.0), Some(300.0))
    );
    assert!(!inner.at_start(0) && inner.at_start(1));
    // Ряд внутри ряда помнит, что снаружи — страницы.
    let deeper = inner.nest_at(10.0).expect("row block size is known");
    assert!(matches!(deeper.kind, Kind::NestedRow(r) if r.outer_paged));
    assert_eq!(Space::columns().nest_at(10.0), None);
    let column = Space {
        block_size: Some(80.0),
        ..Space::columns()
    };
    let at_top = column.nest_at(0.0).expect("known");
    assert!(matches!(at_top.kind, Kind::NestedRow(r) if !r.outer_paged));
    assert!(at_top.at_start(0));
}

#[test]
fn only_pages_paginate_and_only_unsized_columns_balance() {
    let sized = Space {
        block_size: Some(50.0),
        ..Space::columns()
    };
    assert!(Space::pages().is_paginated() && !Space::columns().is_paginated());
    assert!(Space::columns().is_initial_balancing_pass());
    assert!(!sized.is_initial_balancing_pass() && !Space::pages().is_initial_balancing_pass());
    assert_eq!(Space::columns().axis(), StackAxis::Horizontal);
    let vertical = Space {
        wm: wm(true, true, false),
        ..Space::pages()
    };
    assert_eq!(vertical.axis(), StackAxis::VerticalRl);
    assert_eq!(Space::pages().with_not_at_start(2).not_at_start, 2);
}

#[test]
fn repeated_content_is_monolithic() {
    let with = |repeat| Space {
        repeat,
        ..Space::pages()
    };
    assert!(with(RepeatState::None).may_break_inside());
    assert!(!with(RepeatState::Repeat { last: true }).may_break_inside());
    assert!(!with(RepeatState::Inside).may_break_inside());
    let RepeatState::Repeat { last } = with(RepeatState::Repeat { last: false }).repeat else {
        unreachable!("constructed as Repeat");
    };
    assert!(!last);
}

#[test]
fn break_appeal_orders_from_worst_to_perfect() {
    use BreakAppeal::*;
    assert!(LastResort < ViolatingAvoid && ViolatingAvoid < ViolatingOrphansWidows);
    assert!(ViolatingOrphansWidows < Perfect);
    assert_eq!(Space::columns().min_appeal, LastResort);
    assert_eq!(
        Space::columns().with_min_appeal(ViolatingAvoid).min_appeal,
        ViolatingAvoid
    );
}
