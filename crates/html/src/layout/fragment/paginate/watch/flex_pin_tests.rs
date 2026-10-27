//! Закрепление линий flex: рекордер берёт native-индексы final metadata, и те же
//! индексы через Window удерживают членство при росте элемента разрывом.
use super::Watched;
use crate::layout::fragment::flow::blocks::{FlexFrame, FlexLine, Laid};
use gpui::{AvailableSpace, Display, FlexDirection, FlexWrap, LayoutId, Style, point, px, size};

fn line(index: usize, items: &[u64]) -> FlexLine {
    FlexLine {
        index,
        items: items.to_vec(),
        ..Default::default()
    }
}

#[test]
fn every_column_frame_is_pinned_once_by_native_line_index() {
    let rec = Watched::new(&[]);
    let mut laid = Laid::default();
    let frame = |column, lines| FlexFrame { column, lines };
    laid.flexes.insert(10, frame(true, vec![line(0, &[1, 2])]));
    // Wrap-reverse: native индексы убывают в порядке документа.
    laid.flexes
        .insert(20, frame(true, vec![line(1, &[3, 4]), line(0, &[5])]));
    laid.flexes
        .insert(30, frame(false, vec![line(0, &[6]), line(1, &[7])]));
    rec.pin_flex_lines(&laid);
    let pins = rec.flex_lines.borrow().clone();
    let expected: std::collections::HashMap<u64, usize> = [(1, 0), (2, 0), (3, 1), (4, 1), (5, 0)]
        .into_iter()
        .collect();
    assert_eq!(pins, expected);
    // Повторный рост не перезакрепляет уже растущую раскладку.
    laid.flexes
        .insert(10, frame(true, vec![line(0, &[1]), line(1, &[2])]));
    rec.pin_flex_lines(&laid);
    assert_eq!(*rec.flex_lines.borrow(), expected);
}

fn column(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    wrap: FlexWrap,
    heights: &[f32],
) -> (LayoutId, Vec<LayoutId>) {
    let items: Vec<_> = heights
        .iter()
        .map(|&h| {
            window.request_layout(
                Style {
                    size: size(px(10.).into(), px(h).into()),
                    flex_shrink: 0.,
                    ..Style::default()
                },
                [],
                cx,
            )
        })
        .collect();
    let root = window.request_layout(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_wrap: wrap,
            size: size(px(20.).into(), px(100.).into()),
            ..Style::default()
        },
        items.clone(),
        cx,
    );
    (root, items)
}

fn lines_of(window: &mut gpui::Window, items: &[LayoutId]) -> Vec<usize> {
    let (_, flex) = window.parent_flex_layout(items[0]).unwrap();
    items
        .iter()
        .map(|id| {
            flex.lines
                .iter()
                .position(|l| l.items.iter().any(|i| i.id == *id))
                .unwrap()
        })
        .collect()
}

#[gpui::test]
fn metadata_line_indices_hold_membership_while_an_item_grows(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for wrap in [FlexWrap::Wrap, FlexWrap::WrapReverse] {
        for heights in [[40., 40., 40.].as_slice(), &[40., 40.]] {
            cx.draw(
                point(px(0.), px(0.)),
                size(px(400.), px(400.)),
                |window, app| {
                    let space = size(
                        AvailableSpace::Definite(px(100.)),
                        AvailableSpace::Definite(px(100.)),
                    );
                    let (root, items) = column(window, app, wrap, heights);
                    window.compute_layout(root, space, app);
                    let pins = lines_of(window, &items);
                    let mut grown = heights.to_vec();
                    grown[0] = 70.;
                    let (root, items) = column(window, app, wrap, &grown);
                    window.compute_layout(root, space, app);
                    assert_ne!(lines_of(window, &items)[..2], pins[..2]);
                    let (root, items) = column(window, app, wrap, &grown);
                    for (id, line) in items.iter().zip(&pins) {
                        window.set_flex_item_fragmentation_line(*id, Some(*line));
                    }
                    window.compute_layout(root, space, app);
                    assert_eq!(lines_of(window, &items), pins, "{wrap:?} {heights:?}");
                    gpui::div()
                },
            );
        }
    }
}
