//! Границы N/N+1, собственный счёт IFC и продолжение в логических осях.
use super::*;
use crate::layout::fragment::line_shape::line_state;
use crate::style::computed::writing_mode::WritingMode;

fn paragraph(count: usize, orphans: usize, widows: usize) -> Inline {
    Inline {
        lines: (0..count)
            .map(|i| super::super::ContentLine {
                paragraph: 0,
                byte: i,
                state: line_state(&"x".repeat(count), i, i, false).unwrap(),
                top: i as f32 * 10.0,
                bottom: (i + 1) as f32 * 10.0,
                consumed_before: 0.0,
            })
            .collect(),
        orphans,
        widows,
    }
}
fn space(height: f32) -> Space {
    Space {
        block_size: Some(height),
        ..Space::pages()
    }
}

#[test]
fn exact_n_and_n_plus_one_choose_the_next_real_line() {
    for n in [3, 4] {
        let p = paragraph(n, 1, 1);
        let mut store = TokenStore::default();
        let result = p.resume(space(30.0), 0.0, None, &mut store, false).unwrap();
        assert_eq!(result.end, 30.0);
        assert_eq!(result.next.is_some(), n == 4);
        if let Some(id) = result.next {
            let token = store.get(id).unwrap();
            let FcState::Inline(state) = &token.fc else {
                panic!()
            };
            assert_eq!((state.line_idx, state.text_pos), (3, 3));
            assert!(state.past_first_line);
            assert_eq!(token.consumed, 30.0);
        }
    }
}

#[test]
fn widows_roll_back_and_orphans_restart_in_every_fragment() {
    let p = paragraph(11, 2, 2);
    let mut store = TokenStore::default();
    let mut input = None;
    let mut ends = Vec::new();
    loop {
        let result = p
            .resume(space(30.0), 0.0, input, &mut store, false)
            .unwrap();
        ends.push(result.end);
        input = result.next;
        if input.is_none() {
            break;
        }
    }
    assert_eq!(ends, [30.0, 60.0, 90.0, 110.0]);
    let p = paragraph(7, 2, 3);
    let result = p.resume(space(60.0), 0.0, None, &mut store, false).unwrap();
    assert_eq!(result.end, 40.0);
    assert_eq!(result.appeal, BreakAppeal::Perfect);
}

#[test]
fn nonempty_fragment_breaks_before_without_consuming_content() {
    let p = paragraph(6, 2, 2);
    let mut store = TokenStore::default();
    let result = p
        .resume(space(20.0), 10.0, None, &mut store, false)
        .unwrap();
    let token = store.get(result.next.unwrap()).unwrap();
    assert!(token.flags.break_before);
    assert_eq!((result.end, token.consumed, token.seq), (0.0, 0.0, 0));
    let continued = p
        .resume(space(30.0), 0.0, result.next, &mut store, false)
        .unwrap();
    assert_eq!(continued.end, 30.0);
}

#[test]
fn avoid_is_relaxed_after_line_counts_and_tall_lines_stay_whole() {
    let p = paragraph(8, 2, 2);
    let mut store = TokenStore::default();
    let result = p.resume(space(30.0), 0.0, None, &mut store, true).unwrap();
    assert_eq!(result.appeal, BreakAppeal::ViolatingAvoid);
    let result = p.resume(space(5.0), 0.0, None, &mut store, false).unwrap();
    assert_eq!(result.end, 10.0);
    assert_eq!(result.shortage, 5.0);
}

#[test]
fn logical_axes_and_independent_contexts_do_not_change_line_counting() {
    let p = paragraph(5, 2, 2);
    for wm in [
        WritingMode::new(false, false, false, false),
        WritingMode::new(true, true, false, true),
        WritingMode::new(true, false, false, false),
    ] {
        let mut store = TokenStore::default();
        let mut s = space(40.0);
        s.wm = wm;
        let a = p.resume(s, 0.0, None, &mut store, false).unwrap();
        let b = p.resume(s, 0.0, None, &mut store, false).unwrap();
        assert_eq!((a.end, b.end), (30.0, 30.0));
        let c = p.resume(s, 0.0, a.next, &mut store, false).unwrap();
        assert!(c.next.is_none());
    }
}

#[test]
fn addresses_use_logical_utf8_offsets_and_preserve_hard_break_state() {
    let text = "אב a\n字";
    let byte = text.find('字').unwrap();
    let state = line_state(text, 2, byte, true).unwrap();
    assert_eq!(state.text_pos, byte);
    assert!(state.after_hard_break && state.past_first_line);
    assert_eq!(line_state(text, 1, 1, false), Err(Error::InvalidToken));
}

#[test]
fn changed_line_measure_resumes_the_same_content_address() {
    let old = paragraph(8, 2, 2);
    let mut store = TokenStore::default();
    let first = old
        .resume(space(30.0), 0.0, None, &mut store, false)
        .unwrap();
    let mut fresh = paragraph(8, 2, 2);
    for line in &mut fresh.lines {
        line.top *= 2.0;
        line.bottom *= 2.0;
    }
    let next = fresh
        .resume(space(40.0), 0.0, first.next, &mut store, false)
        .unwrap();
    assert_eq!(next.end, 100.0);
    let token = store.get(next.next.unwrap()).unwrap();
    assert_eq!(token.consumed, 70.0);
    assert_eq!(token.seq, 2);
    let FcState::Inline(state) = &token.fc else {
        panic!()
    };
    assert_eq!(state.text_pos, 5);
}
