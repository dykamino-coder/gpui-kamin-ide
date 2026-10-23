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
        gpui::paint_collect_reset();
        let log = Rc::new(RefCell::new(Vec::new()));
        let trace = |phase| {
            PaintOnly(Trace {
                phase,
                log: log.clone(),
            })
            .hooked()
        };
        let mut published = gpui::div()
            .child(gpui::PaintInline::new(trace("inline")))
            .child(gpui::PaintLast::new(trace("positioned")))
            .into_any_element();
        for _ in 0..wrappers {
            let (_, escaped) = Positioning::default().retain().isolate();
            published = escaped.wrap(published);
        }
        let (open, close) = gpui::PaintCollect::pair();
        let root = gpui::div()
            .child(open)
            .child(published)
            .child(trace("float"))
            .child(close);
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(100.), gpui::px(100.)),
            |_, _| root,
        );
        assert_eq!(
            *log.borrow(),
            ["float", "inline", "positioned"],
            "{wrappers} publication wrappers"
        );
    }
}
