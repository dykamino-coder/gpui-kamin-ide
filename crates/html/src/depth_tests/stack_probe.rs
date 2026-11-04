//! Изолированный замер прироста native-стека фаз GPUI на один уровень Div.

use super::*;
use gpui::Styled;
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
struct Samples {
    request: Cell<usize>,
    layout: Cell<usize>,
    prepaint: Cell<usize>,
    paint: Cell<usize>,
}

fn address(anchor: &u8) -> usize {
    std::hint::black_box(anchor) as *const u8 as usize
}

#[test]
#[ignore = "диагностика стека: --ignored --nocapture --test-threads=1"]
fn native_stack_bytes_per_div_level() {
    let mut cx = gpui::TestAppContext::single();
    for block in [false, true] {
        for n in [16, 32, 64, 128] {
            let samples = Rc::new(Samples::default());
            let mut child = Leaf(samples.clone()).into_any_element();
            for _ in 0..n {
                let div = gpui::div().child(child);
                child = if block { div.block() } else { div }.into_any_element();
            }
            // На каждой глубине одинаковы вызывающий frame и измерительный лист.
            // Разность глубин исключает постоянную стоимость измерения.
            let anchor = 0u8;
            let caller = address(&anchor);
            cx.add_empty_window().draw(
                gpui::point(gpui::px(0.), gpui::px(0.)),
                gpui::size(gpui::px(100.), gpui::px(100.)),
                |_, _| FrameProbe(child, "paint".into()),
            );
            for (phase, sample) in [
                ("request", &samples.request),
                ("layout", &samples.layout),
                ("prepaint", &samples.prepaint),
                ("paint", &samples.paint),
            ] {
                assert_ne!(sample.get(), 0, "unvisited {phase}");
                eprintln!(
                    "stack_probe block={block} depth={n} phase={phase} caller_to_leaf_bytes={}",
                    caller.abs_diff(sample.get())
                );
            }
            std::hint::black_box(&anchor);
        }
    }
}

// Проверяет именно paint: остальные фазы пока используют обычный тестовый стек.
#[test]
fn div_paint_native_stack_does_not_grow_with_depth() {
    let mut cx = gpui::TestAppContext::single();
    for block in [false, true] {
        let mut addresses = Vec::new();
        for depth in [16, 128, 512] {
            let samples = Rc::new(Samples::default());
            let mut child = Leaf(samples.clone()).into_any_element();
            for _ in 0..depth {
                let div = gpui::div().child(child);
                child = if block { div.block() } else { div }.into_any_element();
            }
            cx.add_empty_window().draw(
                gpui::point(gpui::px(0.), gpui::px(0.)),
                gpui::size(gpui::px(100.), gpui::px(100.)),
                |_, _| FrameProbe(child, "paint".into()),
            );
            assert_ne!(samples.paint.get(), 0, "paint должен посетить лист");
            addresses.push(samples.paint.get());
        }
        let spread = addresses.iter().max().unwrap() - addresses.iter().min().unwrap();
        assert!(
            spread < 32 * 1024,
            "paint расходует стек с глубиной: {spread} байт"
        );
    }
}

struct Leaf(Rc<Samples>);

impl IntoElement for Leaf {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for Leaf {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        _: &mut gpui::App,
    ) -> (gpui::LayoutId, ()) {
        let anchor = 0u8;
        self.0.request.set(address(&anchor));
        let samples = self.0.clone();
        let id = window.request_measured_layout(gpui::Style::default(), move |_, _, _, _| {
            let anchor = 0u8;
            // Layout может измерять лист повторно; сохраняем самый глубокий вызов.
            let current = address(&anchor);
            let previous = samples.layout.get();
            samples.layout.set(if previous == 0 {
                current
            } else {
                previous.min(current)
            });
            gpui::size(gpui::px(1.), gpui::px(1.))
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut gpui::Window,
        _: &mut gpui::App,
    ) {
        let anchor = 0u8;
        self.0.prepaint.set(address(&anchor));
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut gpui::Window,
        _: &mut gpui::App,
    ) {
        let anchor = 0u8;
        self.0.paint.set(address(&anchor));
    }
}
