//! Принудительный разрыв перед первым элементом линии flex-колонки.
use super::flex_gap_tests::flex_root;
use super::{BlockBox, BlockKid, FlexFrame, FlexLine, Laid, forced_line_start_pending};

/// Две линии колонки: 1, 2 и 3, 4; у 3 разрыв перед ним, верх 3 — `top`.
fn columns(top: f32) -> (BlockBox, Laid) {
    let items = (1..=4)
        .map(|id| BlockKid {
            id,
            force_before: id == 3,
            ..Default::default()
        })
        .collect();
    let root = flex_root(items);
    let mut laid = Laid::of_boxes(&[
        (10, 0.0, 200.0),
        (1, 0.0, 50.0),
        (2, 50.0, 100.0),
        (3, top, top + 50.0),
        (4, top + 50.0, top + 100.0),
    ]);
    laid.margins.insert(3, (top, 0.0));
    let line = |index, items: [u64; 2]| FlexLine {
        index,
        items: items.to_vec(),
        order: items.map(|id| (id as u32, Some(id))).to_vec(),
        ..Default::default()
    };
    laid.flexes.insert(
        10,
        FlexFrame {
            column: true,
            lines: vec![line(0, [1, 2]), line(1, [3, 4])],
        },
    );
    (root, laid)
}

fn starts(top: f32) -> Vec<(u64, f32)> {
    let (root, laid) = columns(top);
    let mut out = Vec::new();
    root.forced_line_starts(&laid, &mut out);
    out
}

#[test]
fn a_separated_line_head_breaks_with_its_margin() {
    // Поле 100 отделяет элемент от края контейнера: разрыв его собственный,
    // перенос — от начала margin box (multi-line-column-039).
    assert_eq!(starts(100.0), vec![(3, 0.0)]);
}

#[test]
fn a_line_head_at_the_container_edge_leaves_the_break_to_the_container() {
    // multi-line-column-029: без отделения граница — край контейнера.
    assert!(starts(0.0).is_empty());
}

#[test]
fn only_an_unbroken_line_head_is_pending() {
    assert!(forced_line_start_pending(0.0, 0.0, 100.0));
    assert!(forced_line_start_pending(30.0, 0.0, 100.0));
    // Продолжение уже начинается с элемента: разрыв случился.
    assert!(!forced_line_start_pending(100.0, 100.0, 200.0));
    assert!(!forced_line_start_pending(100.0, 0.0, 100.0));
}
