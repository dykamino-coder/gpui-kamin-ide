//! Layout IDs stay owned by their probe arena when painting escapes a prepared holder.

use super::*;
use gpui::{Context, IntoElement, Render, RetainedLayout};

struct DeferredView(Rc<Cell<Option<Bounds<Pixels>>>>);

impl Render for DeferredView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut child = kid(Kind::Flow, Default::default(), Default::default());
        let bounds = self.0.clone();
        child.build = Rc::new(move |_, _, _, _, _| {
            gpui::deferred(
                Capture {
                    child: div().w(px(20.)).h(px(10.)).into_any_element(),
                    bounds: bounds.clone(),
                }
                .hooked(),
            )
            .into_any_element()
        });
        BandFlow::new(vec![child], true, &block_cx()).hooked()
    }
}

#[gpui::test]
fn deferred_descendant_keeps_its_prepared_layout_arena(cx: &mut gpui::TestAppContext) {
    let bounds = Rc::new(Cell::new(None));
    let output = bounds.clone();
    let window = cx.add_window(move |_, _| DeferredView(output));
    for _ in 0..3 {
        cx.run_until_parked();
        assert_eq!(bounds.get().unwrap().size, size(px(20.), px(10.)));
        bounds.set(None);
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
    }
}

#[gpui::test]
fn paint_snapshot_reactivates_the_owner_and_can_reenter_the_root_arena(
    cx: &mut gpui::TestAppContext,
) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let mut root_owner = window.retained_layout();
            let mut root = div().w(px(90.)).h(px(30.)).into_any_element();
            let root_id = root.request_layout(window, app);
            root.layout_as_root(
                size(px(100.).into(), AvailableSpace::MaxContent),
                window,
                app,
            );
            let mut arena = RetainedLayout::default();
            let (id, snapshot) = window.with_retained_layout(&mut arena, |window| {
                let mut child = div().w(px(20.)).h(px(10.)).into_any_element();
                let id = child.request_layout(window, app);
                child.layout_as_root(
                    size(px(100.).into(), AvailableSpace::MaxContent),
                    window,
                    app,
                );
                window.with_retained_layout(&mut root_owner, |window| {
                    assert_eq!(
                        window.layout_size_unrounded(root_id),
                        size(px(90.), px(30.))
                    );
                });
                (id, window.paint_ctx())
            });
            window
                .with_paint_ctx(snapshot, |window| {
                    assert_eq!(window.layout_size_unrounded(id), size(px(20.), px(10.)));
                    // Reentering an already active arena must not take its live engine.
                    window.with_retained_layout(&mut arena, |window| {
                        assert_eq!(window.layout_size_unrounded(id), size(px(20.), px(10.)));
                    });
                })
                .unwrap();
            assert_eq!(
                window.layout_size_unrounded(root_id),
                size(px(90.), px(30.))
            );
            div()
        },
    );
}
