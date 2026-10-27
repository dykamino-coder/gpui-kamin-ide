//! Гашение промежутка рядов flex у края фрагментаинера.
use super::{BlockBox, BlockKid, FlexBox, FlexFrame, FlexLine, FlexPush, Laid};

fn rows(force_before_second: bool) -> (BlockBox, Laid) {
    let items = (1..=2)
        .map(|id| BlockKid {
            id,
            monolith: true,
            force_before: id == 2 && force_before_second,
            ..Default::default()
        })
        .collect();
    let root = flex_root(items);
    let mut laid = Laid::of_boxes(&[(10, 0.0, 100.0), (1, 0.0, 50.0), (2, 60.0, 100.0)]);
    let line = |index, start, end, id| FlexLine {
        index,
        start,
        end,
        items: vec![id],
        order: vec![(index as u32, Some(id))],
    };
    laid.flexes.insert(
        10,
        FlexFrame {
            column: false,
            lines: vec![line(0, 0.0, 50.0, 1), line(1, 60.0, 100.0, 2)],
        },
    );
    (root, laid)
}

/// Flex-контейнер `10` с этими элементами.
pub(super) fn flex_root(items: Vec<BlockKid>) -> BlockBox {
    BlockBox {
        id: 10,
        flex: Some(FlexBox {
            items,
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn gaps(b: &BlockBox, laid: &Laid, from: f32, edge: f32) -> Vec<(FlexPush, f32, f32)> {
    let mut out = Vec::new();
    b.suppressed_row_gaps(laid, from, edge, edge, &mut out);
    out
}

#[test]
fn only_the_part_of_a_gap_past_the_edge_is_suppressed() {
    let (b, laid) = rows(false);
    let first = FlexPush {
        container: 10,
        line: 0,
    };
    assert_eq!(gaps(&b, &laid, 0.0, 55.0), vec![(first.clone(), 0.0, 60.0)]);
    assert_eq!(gaps(&b, &laid, 0.0, 50.0), vec![(first, 0.0, 60.0)]);
    // Следующая линия начинается до края: гасить нечего.
    assert!(gaps(&b, &laid, 0.0, 70.0).is_empty());
    // Линия кончилась в прежнем фрагменте.
    assert!(gaps(&b, &laid, 52.0, 152.0).is_empty());
    // Разрыв внутри продолжающегося элемента линии, а не в промежутке.
    let mut out = Vec::new();
    b.suppressed_row_gaps(&laid, 0.0, 30.0, 55.0, &mut out);
    assert!(out.is_empty());
}

#[test]
fn forced_boundaries_keep_their_own_break() {
    let (b, laid) = rows(true);
    assert!(gaps(&b, &laid, 0.0, 55.0).is_empty());
}

#[test]
fn a_line_still_growing_from_a_forced_break_inside_keeps_its_gap() {
    let (mut b, mut laid) = rows(false);
    let inner = BlockBox {
        id: 1,
        kids: [(11, false), (12, true)]
            .map(|(id, force_before)| BlockKid {
                id,
                force_before,
                adjoins: true,
                ..Default::default()
            })
            .to_vec(),
        ..Default::default()
    };
    let item = &mut b.flex.as_mut().unwrap().items[0];
    item.monolith = false;
    item.inner = Some(inner);
    laid.boxes.insert(11, (0.0, 25.0));
    laid.boxes.insert(12, (25.0, 50.0));
    let grows = crate::layout::fragment::push::flex_flow::growths(&b, &laid, 0.0, 50.0, 50.0);
    assert!(
        grows.iter().any(|g| g.amount > 0.0 && g.forced),
        "{grows:?}"
    );
    assert!(!grows.iter().any(|g| g.amount < 0.0), "{grows:?}");
}
