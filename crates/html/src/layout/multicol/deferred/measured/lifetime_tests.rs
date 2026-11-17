//! Prepared columns expire after paint; transferred arenas remain owned by their consumer.

use super::*;
use crate::layout::fragment::types::StackAxis;
use gpui::{point, px, size};
use std::cell::RefCell;

#[gpui::test]
fn frame_expiry_breaks_the_recipe_cycle_without_releasing_transferred_columns(
    cx: &mut gpui::TestAppContext,
) {
    for consumed in [false, true] {
        let marker = Rc::new(());
        let weak = Rc::downgrade(&marker);
        let transferred = RefCell::new(None);
        cx.add_empty_window().draw(
            point(px(0.), px(0.)),
            size(px(100.), px(100.)),
            |window, app| {
                let tree = crate::document::parsed::parse(
                    "<section style='column-count:2'><div style='height:50px'></div></section>",
                    "",
                );
                let opts = super::super::tests::materialized_opts(&tree, (100., 100.));
                let host = super::super::tests::section(&tree);
                let merged = crate::style::cascade::materialize::box_style(
                    &opts.root_style(),
                    &host.style,
                    opts.cx.vcx(),
                );
                let recipe =
                    Recipe::new(host, &merged, &opts, 2, None, StackAxis::Horizontal, None)
                        .expect("deferred recipe");
                let mut engine = RetainedLayout::default();
                window.with_retained_layout(&mut engine, |window| {
                    let captured = recipe.clone();
                    window.request_measured_layout(Default::default(), move |_, _, _, _| {
                        let _ = (&captured, &marker);
                        size(px(1.), px(1.))
                    });
                });
                recipe.keep_for_frame(
                    Some(Measured {
                        width: 100_f32.to_bits(),
                        col_w: 50_f32.to_bits(),
                        children: Vec::new(),
                        geometry: Default::default(),
                        copies: 2,
                        forced_h: None,
                        balance_failure: None,
                        extent: (12., None, None),
                        engine,
                    }),
                    app,
                );
                assert_eq!(recipe.measured_at(100.).unwrap().0, 12.);
                assert!(weak.upgrade().is_some());
                if consumed {
                    transferred.replace(recipe.take_measured(50.));
                }
                gpui::div()
            },
        );
        cx.run_until_parked();
        assert_eq!(weak.upgrade().is_some(), consumed);
        drop(transferred.take());
        assert!(weak.upgrade().is_none());
    }
}
