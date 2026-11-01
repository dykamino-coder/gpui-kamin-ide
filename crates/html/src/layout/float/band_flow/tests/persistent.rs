//! Повторный построитель узла использует постоянную пару, dirty требует нового замера.

use super::*;
use gpui::InteractiveElement;

#[gpui::test]
fn deferred_preparation_preserves_identified_ancestors(cx: &mut gpui::TestAppContext) {
    let builds = Rc::new(Cell::new(0));
    let bounds = Rc::new(Cell::new(None));
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            let mut child = kid(Kind::Flow, builds.clone(), bounds.clone());
            let build = child.build.clone();
            child.build = Rc::new(move |cb, avail, shapes, height, measure| {
                div()
                    .id("prepared-child")
                    .child(build(cb, avail, shapes, height, measure))
                    .into_any_element()
            });
            div()
                .id("ancestor")
                .child(BandFlow::new(vec![child], true, &block_cx()).hooked())
        });
    assert_eq!(builds.get(), 1);
    assert_eq!(bounds.get().unwrap().size, size(px(20.), px(10.)));
}

#[gpui::test]
fn intrinsic_slot_survives_rebuilding_the_host(cx: &mut gpui::TestAppContext) {
    let builds = Rc::new(Cell::new(0));
    let slot = crate::dom::IntrinsicSizes::default();
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            for pass in 0..3 {
                if pass == 2 {
                    slot.set(None);
                }
                let mut child = kid(Kind::Flow, builds.clone(), Default::default());
                child.intrinsic = slot.clone();
                let mut flow = BandFlow::new(vec![child], true, &block_cx()).hooked();
                let root = flow.request_layout(window, app);
                for width in [AvailableSpace::MinContent, AvailableSpace::MaxContent] {
                    let actual = window.measure_layout_size(
                        root,
                        size(width, AvailableSpace::MaxContent),
                        gpui::Axis::Horizontal,
                        app,
                    );
                    assert_eq!(actual.width, px(20.));
                }
                assert_eq!(builds.get(), if pass == 2 { 2 } else { 1 });
            }
            div()
        },
    );
}

#[test]
fn auto_flex_and_grid_items_leave_intrinsic_sizing_to_their_owner() {
    use crate::layout::block::available_width::inner;
    use crate::style::computed::{Computed, Display};
    for display in [Display::Flex, Display::Grid] {
        let parent = Computed {
            display: Some(display),
            ..Default::default()
        };
        let item = Computed {
            flex_item: true,
            ..Default::default()
        };
        let parent_width = inner(&parent, Some(100.));
        assert_eq!(parent_width, None);
        assert_eq!(inner(&item, parent_width), None);
    }
}

#[gpui::test]
fn cloned_deferred_context_survives_deep_ancestry_and_nested_restore(
    cx: &mut gpui::TestAppContext,
) {
    fn capture(
        window: &mut Window,
        depth: u64,
    ) -> (gpui::ElementLayoutContext, gpui::GlobalElementId) {
        if depth > 0 {
            window.with_id(depth, |window| capture(window, depth - 1))
        } else {
            let context = window.element_layout_context();
            let id = window.with_global_id(999u64.into(), |id, _| id.clone());
            (context, id)
        }
    }
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, _| {
            let (context, expected) = capture(window, 40);
            let snapshot = context.clone();
            drop(context);
            let outer = window.with_global_id(999u64.into(), |id, _| id.clone());
            for _ in 0..2 {
                window.with_element_layout_context(&snapshot, |window| {
                    window.with_global_id(999u64.into(), |id, _| assert_eq!(*id, expected));
                    window.with_id("temporary", |window| {
                        window.with_element_layout_context(&snapshot, |window| {
                            window.with_global_id(999u64.into(), |id, _| assert_eq!(*id, expected));
                        });
                    });
                    window.with_global_id(999u64.into(), |id, _| assert_eq!(*id, expected));
                });
                window.with_global_id(999u64.into(), |id, _| assert_eq!(*id, outer));
            }
            div()
        },
    );
}
