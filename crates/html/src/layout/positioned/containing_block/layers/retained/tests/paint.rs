//! Публикация проб сохраняет фазы краски флоатов, строк и позиционированных.

use super::super::*;
use crate::hooks::{IntoHooked, PaintOnly, Painter};
use gpui::{IntoElement, ParentElement};

struct Trace {
    phase: &'static str,
    log: Rc<RefCell<Vec<&'static str>>>,
}

impl Painter for Trace {
    type Prepaint = ();

    fn style(&self) -> gpui::Style {
        gpui::Style::default()
    }

    fn paint(
        &mut self,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut gpui::LayoutId,
        _: &mut (),
        _: &mut gpui::Window,
        _: &mut gpui::App,
    ) {
        self.log.borrow_mut().push(self.phase);
    }
}

#[gpui::test]
fn published_probe_keeps_inline_and_positioned_paint_after_floats(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    // CSS 2.1 Appendix E: float (5), inline foreground (7), positioned (8).
    // Как в stack-floats-001/002: ранняя строка перекрывает поздний float.
    for wrappers in [1, 2] {
        let log = Rc::new(RefCell::new(Vec::new()));
        let trace = |phase| {
            PaintOnly(Trace {
                phase,
                log: log.clone(),
            })
            .hooked()
        };
        let mut published = gpui::div()
            .child(crate::hooks::paint_inline(trace("inline")))
            .child(crate::hooks::paint_last(trace("positioned"), 0))
            .into_any_element();
        for _ in 0..wrappers {
            let (_, escaped) = Positioning::default().retain().isolate();
            published = escaped.wrap(published);
        }
        let (open, close) = crate::hooks::paint_collect();
        let root = gpui::div()
            .child(open)
            .child(published)
            .child(trace("float"))
            .child(close);
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(100.), gpui::px(100.)),
            |_, _| crate::hooks::paint_root(root.into_any_element()),
        );
        assert_eq!(
            *log.borrow(),
            ["float", "inline", "positioned"],
            "{wrappers} publication wrappers"
        );
    }
}

#[gpui::test]
fn published_positioned_root_keeps_its_phase_without_collector(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for wrappers in [1, 2] {
        let log = Rc::new(RefCell::new(Vec::new()));
        let trace = |phase| {
            PaintOnly(Trace {
                phase,
                log: log.clone(),
            })
            .hooked()
        };
        let mut published = crate::hooks::paint_last(trace("positioned"), 0);
        for _ in 0..wrappers {
            let (_, escaped) = Positioning::default().retain().isolate();
            published = escaped.wrap(published);
        }
        // Закрытый собиратель у колонки: CSS visitor откладывает PaintLast.
        // Публикация не должна превратить его в обычного раннего ребёнка.
        let root = gpui::div().child(published).child(trace("normal"));
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(100.), gpui::px(100.)),
            |_, _| crate::hooks::paint_root(root.into_any_element()),
        );
        assert_eq!(
            *log.borrow(),
            ["normal", "positioned"],
            "{wrappers} wrappers"
        );
    }
}
