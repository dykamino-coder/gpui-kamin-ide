//! Кандидаты страниц: независимые IFC, общие края и адреса продолжения.

use crate::layout::fragment::flow::{ContentLine, inline::Inline};
use crate::layout::fragment::line_shape::line_state;
use crate::layout::fragment::plan::{Measurement, Plan, Space, fill_resumed};
use crate::layout::fragment::types::Kid;

fn paragraph(count: usize, ordinal: usize, offset: f32) -> Inline {
    let text = "x".repeat(count);
    Inline {
        lines: (0..count)
            .map(|i| ContentLine {
                paragraph: ordinal,
                byte: i,
                state: line_state(&text, i, i, false).unwrap(),
                top: offset + i as f32 * 10.0,
                bottom: offset + (i + 1) as f32 * 10.0,
                consumed_before: 0.0,
            })
            .collect(),
        orphans: 2,
        widows: 2,
    }
}
fn kid(count: usize) -> Kid {
    let h = count as f32 * 10.0;
    Kid {
        h,
        over: h,
        cuts: (1..=count)
            .map(|i| (i as f32 * 10.0, i as f32 * 10.0))
            .collect(),
        ..Default::default()
    }
}
fn geometry(paragraphs: Vec<Inline>) -> crate::layout::fragment::flow::inline::Geometry {
    crate::layout::fragment::flow::inline::Geometry {
        paragraphs,
        ..Default::default()
    }
}
fn fill(k: Kid, inline: Vec<Inline>, height: f32) -> Plan {
    fill_resumed(
        &[k],
        &|_| crate::layout::fragment::token::Space {
            block_size: Some(height),
            ..crate::layout::fragment::token::Space::pages()
        },
        Space::pages(8),
        &|_, _| None,
        &mut |_| Measurement {
            copy: None,
            inline: crate::layout::fragment::flow::inline::Geometry {
                paragraphs: inline.clone(),
                ..Default::default()
            },
        },
    )
    .unwrap()
    .2
}
#[test]
fn page_candidates_restart_counts_and_roll_back_for_widows() {
    let plan = fill(kid(10), vec![paragraph(10, 0, 0.0)], 30.0);
    assert_eq!(
        plan.iter().map(|f| (f.from, f.h)).collect::<Vec<_>>(),
        [(0.0, 30.0), (30.0, 30.0), (60.0, 20.0), (80.0, 20.0)]
    );
}
#[test]
fn forced_boundary_wins_over_avoid_and_line_counts() {
    let mut k = kid(6);
    k.forced = vec![10.0];
    k.keep = vec![(0.0, 60.0)];
    let plan = fill(k, vec![paragraph(6, 0, 0.0)], 40.0);
    assert_eq!((plan[0].from, plan[0].h), (0.0, 10.0));
    assert_eq!(plan[1].from, 10.0);
}
#[test]
fn neighboring_ifcs_have_separate_orphan_counts() {
    let plan = fill(
        kid(7),
        vec![paragraph(3, 0, 0.0), paragraph(4, 1, 30.0)],
        40.0,
    );
    assert_eq!(plan[0].h, 30.0);
    assert_eq!(plan[1].from, 30.0);
}

#[test]
fn tall_line_reserves_physical_pages_before_the_next_paragraph() {
    let mut first = paragraph(1, 0, 0.0);
    first.lines[0].bottom = 350.0;
    let mut k = kid(36);
    k.cuts = vec![(350.0, 350.0), (360.0, 360.0)];
    k.solid = vec![(0.0, 350.0)];
    let plan = fill(k, vec![first, paragraph(1, 1, 350.0)], 100.0);
    assert_eq!(
        plan.iter()
            .map(|f| (f.col, f.from, f.h))
            .collect::<Vec<_>>(),
        [
            (0, 0.0, 100.0),
            (1, 100.0, 100.0),
            (2, 200.0, 100.0),
            (3, 300.0, 60.0)
        ]
    );
}

#[test]
fn committed_candidate_carries_the_token_through_changed_metrics() {
    use super::{Tokens, cut};
    use crate::layout::fragment::token::fc::FcState;
    let mut tokens = Tokens::default();
    let mut shortage = f32::MAX;
    let mut p = paragraph(8, 0, 0.0);
    assert_eq!(
        cut(
            &mut tokens,
            &geometry(vec![p.clone()]),
            0.0,
            30.0,
            false,
            &mut shortage
        )
        .unwrap(),
        Some(Some((30.0, 30.0)))
    );
    assert!(tokens.next.is_empty());
    tokens.commit(30.0);
    let first = tokens.store.get(tokens.next[&0]).unwrap();
    assert_eq!((first.seq, first.consumed), (1, 30.0));
    for line in &mut p.lines {
        line.top *= 2.0;
        line.bottom *= 2.0;
    }
    assert_eq!(
        cut(
            &mut tokens,
            &geometry(vec![p]),
            60.0,
            40.0,
            false,
            &mut shortage
        )
        .unwrap(),
        Some(Some((100.0, 100.0)))
    );
    tokens.commit(100.0);
    let t = tokens.store.get(tokens.next[&0]).unwrap();
    assert_eq!((t.seq, t.consumed), (2, 70.0));
    assert!(matches!(&t.fc, FcState::Inline(s) if s.text_pos == 5));
}

#[test]
fn parallel_ifcs_only_cut_at_a_shared_line_boundary() {
    let mut a = paragraph(12, 0, 0.0);
    let mut b = paragraph(8, 1, 0.0);
    a.orphans = 1;
    a.widows = 1;
    b.orphans = 1;
    b.widows = 1;
    for line in &mut b.lines {
        line.top *= 1.5;
        line.bottom *= 1.5;
    }
    let plan = fill(kid(12), vec![a, b], 45.0);
    assert_eq!(plan[0].h, 30.0);
    assert_eq!(plan[1].from, 30.0);
}
#[test]
fn structural_barrier_rolls_back_and_rechecks_line_boundaries() {
    let mut g = geometry(vec![paragraph(10, 0, 0.0)]);
    g.barriers = vec![(25.0, 45.0)];
    let k = kid(10);
    let plan = fill_resumed(
        &[k],
        &|_| crate::layout::fragment::token::Space {
            block_size: Some(40.0),
            ..crate::layout::fragment::token::Space::pages()
        },
        Space::pages(8),
        &|_, _| None,
        &mut |_| Measurement {
            copy: None,
            inline: g.clone(),
        },
    )
    .unwrap()
    .2;
    assert_eq!(plan[0].h, 20.0);
}

#[test]
fn avoid_rollback_rechecks_crossing_and_previously_finished_ifcs() {
    use super::{Tokens, cut};
    use crate::layout::fragment::token::fc::FcState;
    for (room, end) in [(40.0, 45.0), (110.0, 125.0)] {
        let mut tokens = Tokens::default();
        let mut g = geometry(vec![paragraph(10, 0, 0.0)]);
        g.keep = vec![(25.0, end)];
        let mut shortage = f32::MAX;
        assert_eq!(
            cut(&mut tokens, &g, 0.0, room, false, &mut shortage).unwrap(),
            Some(Some((20.0, 20.0)))
        );
        tokens.commit(20.0);
        let t = tokens.store.get(tokens.next[&0]).unwrap();
        assert_eq!((t.seq, t.consumed), (1, 20.0));
        assert!(matches!(&t.fc, FcState::Inline(state) if state.text_pos == 2));
    }
}
