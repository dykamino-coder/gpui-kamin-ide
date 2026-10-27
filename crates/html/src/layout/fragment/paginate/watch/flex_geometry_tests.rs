//! Кадры flex рекордера: членство и порядок — из финального прохода решателя,
//! а не из совпадения координат; native-индекс линии сохраняется.
use super::Watched;
use crate::layout::fragment::flow::blocks::{BlockBox, BlockKid, FlexBox, FlexFrame};
use crate::layout::fragment::geometry::Node;
use crate::layout::fragment::types::StackAxis;
use gpui::{AlignItems, AvailableSpace, Display, FlexDirection, FlexWrap, Style, point, px, size};

/// Flex `10` с элементами `1..`: рекордер видит те же узлы, что записал бы `Watch`.
fn frame(
    cx: &mut gpui::VisualTestContext,
    container: Style,
    items: &[(f32, f32)],
) -> (FlexFrame, Vec<(f32, f32)>) {
    let blocks = BlockBox {
        id: 10,
        flex: Some(FlexBox {
            items: (1..=items.len() as u64)
                .map(|id| BlockKid {
                    id,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let rec = Watched::for_blocks_axis(blocks, StackAxis::Horizontal, false);
    let mut out = None;
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let ids: Vec<_> = items
                .iter()
                .map(|&(w, h)| {
                    window.request_layout(
                        Style {
                            size: size(px(w).into(), px(h).into()),
                            flex_shrink: 0.,
                            ..Style::default()
                        },
                        [],
                        app,
                    )
                })
                .collect();
            let root = window.request_layout(container, ids.clone(), app);
            for (id, lid) in (1..).zip(&ids) {
                rec.seen
                    .borrow_mut()
                    .insert(id, Node::capture(*lid, window));
            }
            rec.seen
                .borrow_mut()
                .insert(10, Node::capture(root, window));
            let space = AvailableSpace::Definite(px(400.));
            window.compute_layout(root, size(space, space), app);
            let laid = rec.laid_axis(
                point(px(0.), px(0.)),
                size(px(400.), px(400.)),
                StackAxis::Horizontal,
                window,
            );
            let boxes = (1..=items.len() as u64)
                .map(|id| laid.at(id).unwrap())
                .collect();
            out = Some((laid.flexes[&10].clone(), boxes));
            gpui::div()
        },
    );
    out.unwrap()
}

fn sorted(mut ids: Vec<u64>) -> Vec<u64> {
    ids.sort_unstable();
    ids
}

#[gpui::test]
fn row_lines_follow_the_block_axis_and_keep_solver_membership(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for direction in [FlexDirection::Row, FlexDirection::RowReverse] {
        for (wrap, top_line) in [(FlexWrap::Wrap, 0), (FlexWrap::WrapReverse, 1)] {
            let container = Style {
                display: Display::Flex,
                flex_direction: direction,
                flex_wrap: wrap,
                align_items: Some(AlignItems::Center),
                size: size(px(100.).into(), gpui::Length::Auto),
                ..Style::default()
            };
            let items = [(40., 20.), (40., 40.), (40., 20.), (40., 30.)];
            let (frame, boxes) = frame(cx, container, &items);
            assert!(!frame.column);
            assert_eq!(frame.lines.len(), 2);
            // Ряды упорядочены по блочной оси; native-индекс wrap-reverse обратный.
            assert!(frame.lines[0].end <= frame.lines[1].start);
            assert_eq!(frame.lines[0].index, 0);
            let first = &frame.lines[top_line];
            assert_eq!(sorted(first.items.clone()), [1, 2]);
            assert_eq!(first.order, [(0, Some(1)), (1, Some(2))]);
            assert_eq!(sorted(frame.lines[1 - top_line].items.clone()), [3, 4]);
            // Выравнивание разводит верх элементов одной линии.
            assert_ne!(boxes[0].0, boxes[1].0);
        }
    }
}

#[gpui::test]
fn column_lines_keep_native_order_and_item_sequence(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for direction in [FlexDirection::Column, FlexDirection::ColumnReverse] {
        for wrap in [FlexWrap::Wrap, FlexWrap::WrapReverse] {
            let container = Style {
                display: Display::Flex,
                flex_direction: direction,
                flex_wrap: wrap,
                size: size(gpui::Length::Auto, px(100.).into()),
                ..Style::default()
            };
            let (frame, boxes) = frame(cx, container, &[(20., 60.), (30., 30.), (20., 50.)]);
            assert!(frame.column);
            // Параллельные колонки не пересортировываются: индекс — порядок
            // final metadata, при wrap-reverse первая линия документа последняя.
            let lines: Vec<_> = frame.lines.iter().map(|l| l.index).collect();
            assert_eq!(lines, [0, 1]);
            let first = &frame.lines[usize::from(wrap == FlexWrap::WrapReverse)];
            assert_eq!(sorted(first.items.clone()), [1, 2]);
            assert_eq!(first.order, [(0, Some(1)), (1, Some(2))]);
            assert_eq!(first.end - first.start, 90.);
            let reverse = direction == FlexDirection::ColumnReverse;
            assert_eq!(boxes[0].0 < boxes[1].0, !reverse);
        }
    }
}
