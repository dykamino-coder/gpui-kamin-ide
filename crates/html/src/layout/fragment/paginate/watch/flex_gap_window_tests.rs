//! Погашенный промежуток через Window: следующая линия сдвигается к началу, контейнер по
//! содержимому сжимается, заданный размер и выравнивание сохраняются.
use gpui::{AlignContent, AvailableSpace, Display, FlexWrap, LayoutId, Style, point, px, size};

fn rows(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    container: Style,
) -> (LayoutId, Vec<LayoutId>) {
    let items: Vec<_> = (0..2)
        .map(|_| {
            window.request_layout(
                Style {
                    size: size(px(40.).into(), px(50.).into()),
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
            ..container
        },
        items.clone(),
        cx,
    );
    (root, items)
}

fn bounds(window: &mut gpui::Window, id: LayoutId) -> gpui::Bounds<gpui::Pixels> {
    crate::layout::fragment::geometry::Node::capture(id, window)
        .bounds(window)
        .unwrap()
}

fn top(window: &mut gpui::Window, id: LayoutId) -> f32 {
    f32::from(bounds(window, id).origin.y)
}

#[gpui::test]
fn negative_line_growth_suppresses_the_gap_after_alignment(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let auto = Style {
        size: size(px(40.).into(), gpui::Length::Auto),
        gap: size(px(0.).into(), px(100.).into()),
        align_content: Some(AlignContent::FlexStart),
        ..Style::default()
    };
    let definite = Style {
        size: size(px(40.).into(), px(300.).into()),
        align_content: Some(AlignContent::SpaceBetween),
        ..Style::default()
    };
    for (style, second, height) in [(auto, 150., 150.), (definite, 250., 300.)]
        .into_iter()
        .flat_map(|(s, a, h)| {
            [FlexWrap::Wrap, FlexWrap::WrapReverse].map(|flex_wrap| {
                (
                    Style {
                        flex_wrap,
                        ..s.clone()
                    },
                    a,
                    h,
                )
            })
        })
    {
        // Индекс линии — физический: верхняя линия wrap-reverse — последняя
        // по порядку документа.
        let (upper, lower) = if style.flex_wrap == FlexWrap::Wrap {
            (0, 1)
        } else {
            (1, 0)
        };
        cx.draw(
            point(px(0.), px(0.)),
            size(px(400.), px(400.)),
            |window, app| {
                let space = size(
                    AvailableSpace::Definite(px(40.)),
                    AvailableSpace::MaxContent,
                );
                let (root, items) = rows(window, app, style.clone());
                window.compute_layout(root, space, app);
                assert_eq!(top(window, items[lower]), second);
                let (root, items) = rows(window, app, style.clone());
                window.set_flex_line_fragmentation_growth(root, &[-50.]);
                window.compute_layout(root, space, app);
                assert_eq!(top(window, items[upper]), 0.);
                assert_eq!(top(window, items[lower]), second - 50.);
                assert_eq!(f32::from(bounds(window, root).size.height), height);
                gpui::div()
            },
        );
    }
}
