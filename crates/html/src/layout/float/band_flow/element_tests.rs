//! Тесты: measure-колбэк видит, какие оси финального прохода родитель уже
//! зафиксировал, а пробы — наследуемый стиль текста запросившего элемента.

use gpui::{
    AlignItems, AvailableSpace, Display, FlexDirection, Pixels, Size, Style, point, px, size,
};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "work-counters")]
#[path = "stretch_tests.rs"]
mod stretch;

type Calls = Rc<RefCell<Vec<(Size<bool>, AvailableSpace)>>>;

fn column(window: &mut gpui::Window, app: &mut gpui::App, align: Option<AlignItems>) -> Calls {
    let calls: Calls = Rc::default();
    let seen = calls.clone();
    let child = window.request_measured_layout(Style::default(), move |_, available, window, _| {
        seen.borrow_mut()
            .push((window.measured_size_fixed(), available.width));
        size(px(40.), px(10.))
    });
    let root = window.request_layout(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: align,
            size: size(px(300.).into(), px(200.).into()),
            ..Style::default()
        },
        [child],
        app,
    );
    let space = size(
        AvailableSpace::Definite(px(300.)),
        AvailableSpace::Definite(px(200.)),
    );
    window.compute_layout(root, space, app);
    assert_eq!(window.measured_size_fixed(), Size::default());
    calls
}

fn definite(space: AvailableSpace) -> Option<Pixels> {
    match space {
        AvailableSpace::Definite(w) => Some(w),
        _ => None,
    }
}

/// Sizing probes see no fixed axis; only the last (final) call does, with the
/// used content-box inline size as its definite available space.
fn assert_final_call_fixed(calls: &Calls, inline: f32) {
    let calls = calls.borrow();
    let (last, probes) = calls.split_last().expect("measured");
    assert!(last.0.width && last.0.height, "{calls:?}");
    assert_eq!(definite(last.1), Some(px(inline)), "{calls:?}");
    assert!(
        probes.iter().all(|(f, _)| !f.width && !f.height),
        "{calls:?}"
    );
}

#[gpui::test]
fn stretched_column_item_reports_the_parent_fixed_inline_size(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            assert_final_call_fixed(&column(window, app, None), 300.);
            gpui::div()
        },
    );
}

#[gpui::test]
fn start_aligned_column_item_is_fixed_at_its_fit_content_size(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let calls = column(window, app, Some(AlignItems::FlexStart));
            assert_final_call_fixed(&calls, 40.);
            assert!(
                calls
                    .borrow()
                    .iter()
                    .any(|(_, w)| matches!(w, AvailableSpace::MaxContent))
            );
            gpui::div()
        },
    );
}

/// A measure callback and its probes see the requesting element's text
/// style, while the nested layout leaves no fixed axes of the outer leaf behind.
#[gpui::test]
fn probes_inherit_the_requesting_element_text_style(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let seen = Rc::new(std::cell::Cell::new(Vec::new()));
            let record = seen.clone();
            let big = gpui::TextStyleRefinement {
                font_size: Some(px(31.).into()),
                ..Default::default()
            };
            let leaf = window.with_text_style(Some(big), |window| {
                let scope = window.element_layout_context();
                window.request_measured_layout(Style::default(), move |_, _, window, _| {
                    let outer = window.text_style().font_size;
                    let (probe, fixed) = window.with_element_layout_context(&scope, |window| {
                        window.with_nested_layout(|window| {
                            (window.text_style().font_size, window.measured_size_fixed())
                        })
                    });
                    let mut calls = record.take();
                    calls.push((outer, probe, fixed));
                    record.set(calls);
                    size(px(10.), px(10.))
                })
            });
            let root = window.request_layout(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    size: size(px(300.).into(), px(200.).into()),
                    ..Style::default()
                },
                [leaf],
                app,
            );
            let space = size(
                AvailableSpace::Definite(px(300.)),
                AvailableSpace::Definite(px(200.)),
            );
            window.compute_layout(root, space, app);
            let seen = seen.take();
            assert!(!seen.is_empty());
            for (outer, probe, fixed) in seen.iter() {
                assert_eq!(*outer, px(31.).into(), "{seen:?}");
                assert_eq!(*probe, px(31.).into(), "{seen:?}");
                assert_eq!(*fixed, Size::default(), "{seen:?}");
            }
            gpui::div()
        },
    );
}
