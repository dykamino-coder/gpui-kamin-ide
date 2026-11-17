//! Frame expiry breaks measure-arena cycles without discarding numeric plans.

use super::*;
use crate::layout::float::band_flow::{Plan, Slot, cache::Plans};
use gpui::IntoElement;

#[gpui::test]
fn unused_nested_prepared_children_expire_while_the_plan_remains_cached(
    cx: &mut gpui::TestAppContext,
) {
    let cache = Rc::new(Plans::default());
    let marker = Rc::new(());
    let weak = Rc::downgrade(&marker);
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let mut arena = RetainedLayout::default();
            window.with_retained_layout(&mut arena, |window| {
                let captured = cache.clone();
                window.request_measured_layout(Default::default(), move |_, _, _, _| {
                    let _ = (&captured, &marker);
                    size(px(1.), px(1.))
                });
            });
            let prepared = Prepared::new(
                gpui::div().into_any_element(),
                arena,
                window.element_id_context(),
                size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                size(px(20.), px(10.)),
            );
            let plan = cache.insert_for_frame(
                Plan {
                    width: 100.,
                    height: 12.,
                    slots: vec![Slot {
                        kids: vec![Slot {
                            prepared,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                },
                app,
            );
            assert!(weak.upgrade().is_some());
            assert_eq!(
                format!("{:?}", plan.slots[0].kids[0].prepared),
                "Prepared(true)"
            );
            gpui::div()
        },
    );
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());
    let plan = cache.get(100.).expect("numeric plan survives the frame");
    assert_eq!(plan.height, 12.);
    assert!(plan.slots[0].kids[0].prepared.take(None).is_none());
}
