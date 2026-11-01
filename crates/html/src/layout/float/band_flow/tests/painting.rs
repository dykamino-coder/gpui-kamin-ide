//! Удержанный ребёнок сохраняет атомарную краску float и экспорт positioned-потомков.

use super::*;
use crate::hooks::{paint_collect, paint_inline, paint_last, paint_root};
use std::cell::RefCell;

type Events = Rc<RefCell<Vec<&'static str>>>;

fn mark(name: &'static str, events: Events) -> AnyElement {
    gpui::canvas(
        |_, _, _| (),
        move |_, (), _, _| events.borrow_mut().push(name),
    )
    .w(px(20.))
    .h(px(10.))
    .into_any_element()
}

#[gpui::test]
fn prepared_float_drains_inline_locally_and_exports_positioned_content(
    cx: &mut gpui::TestAppContext,
) {
    let events = Events::default();
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            let float = Kind::Float {
                side: 1,
                clear: None,
                shrink: false,
                letter: false,
                own_layout: false,
            };
            let mut first = kid(float, Default::default(), Default::default());
            let output = events.clone();
            first.build = Rc::new(move |_, _, _, _, _| {
                div()
                    .child(paint_inline(mark("first-inline", output.clone())))
                    .child(paint_last(mark("positioned", output.clone()), 1))
                    .into_any_element()
            });
            let mut second = kid(float, Default::default(), Default::default());
            let output = events.clone();
            second.build = Rc::new(move |_, _, _, _, _| mark("second-float", output.clone()));
            let mut block = kid(Kind::Flow, Default::default(), Default::default());
            let output = events.clone();
            block.build = Rc::new(move |_, _, _, _, _| mark("block", output.clone()));
            // paint_root задаёт проход, а парные метки открывают stacking context,
            // как в дереве документа: atomic float экспортирует positioned в него.
            let (open, close) = paint_collect();
            paint_root(
                div()
                    .child(open)
                    .child(BandFlow::new(vec![first, second, block], true, &block_cx()).hooked())
                    .child(close)
                    .into_any_element(),
            )
        });
    assert_eq!(
        *events.borrow(),
        ["block", "first-inline", "second-float", "positioned"]
    );
}
