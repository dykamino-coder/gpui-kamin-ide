//! Глубина допущенных узлов не расходует native-стек prepaint.

use super::*;
use crate::hooks::Wrap;
use gpui::{IntoElement, TestAppContext, canvas, point, px, size};
use std::cell::Cell;

struct SegmentRoot(AnyElement);

impl Wrap for SegmentRoot {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.0
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        window
            .with_prepaint_segments_async(
                bounds,
                Rc::from([]),
                cx,
                stack,
                async |window, cx, stack| {
                    self.0.prepaint_async(window, cx, stack).await;
                },
            )
            .await
            .unwrap();
    }

    async fn paint_async(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
        _: &mut gpui::PaintStack,
    ) {
    }
}

#[test]
fn admitted_prepaint_native_stack_does_not_grow_with_depth() {
    let mut app = TestAppContext::single();
    let mut addresses = Vec::new();
    for depth in [16, 64, 128] {
        let sample = Rc::new(Cell::new(0));
        app.add_empty_window()
            .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
                let sample = sample.clone();
                let mut inner = canvas(
                    move |_, _, _| {
                        let anchor = 0u8;
                        sample.set(std::hint::black_box(&anchor) as *const u8 as usize);
                    },
                    |_, (), _, _| {},
                )
                .into_any_element();
                let rec = Rc::new(Watched::default());
                rec.set_admission([(1, Rc::from([]))].into());
                for _ in 0..depth {
                    inner = Watch {
                        slot: Slot::Box(1),
                        inner,
                        rec: rec.clone(),
                    }
                    .hooked();
                }
                SegmentRoot(inner).into_hooked()
            });
        assert_ne!(sample.get(), 0, "prepaint должен посетить лист");
        addresses.push(sample.get());
    }
    let spread = addresses.iter().max().unwrap() - addresses.iter().min().unwrap();
    assert!(
        spread < 32 * 1024,
        "prepaint расходует стек с глубиной: {spread}"
    );
    app.quit();
}
