//! KaminIDE patch: tests of views redrawn alone (`isolated`).

use crate::{
    Context, Entity, StyleRefinement, TestAppContext, Window, div, isolated, point, prelude::*,
    px, size,
};
use std::cell::Cell;
use std::rc::Rc;

struct Label {
    renders: Rc<Cell<usize>>,
    text: &'static str,
}

impl Render for Label {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .debug_selector(|| "LABEL".into())
            .size_full()
            .child(self.text)
    }
}

struct Panel {
    renders: Rc<Cell<usize>>,
    label: Entity<Label>,
}

impl Render for Panel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .size_full()
            .pl(px(30.))
            .pt(px(20.))
            .child(isolated(self.label.clone()).w(px(40.)).h(px(10.)))
    }
}

struct Root {
    renders: Rc<Cell<usize>>,
    panel: Entity<Panel>,
    cached: bool,
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let panel = if self.cached {
            self.panel
                .clone()
                .cached(StyleRefinement::default().size_full())
                .into_any_element()
        } else {
            self.panel.clone().into_any_element()
        };
        div().size_full().child(panel)
    }
}

struct Counts {
    root: Rc<Cell<usize>>,
    panel: Rc<Cell<usize>>,
    label: Rc<Cell<usize>>,
}

impl Counts {
    fn get(&self) -> (usize, usize, usize) {
        (self.root.get(), self.panel.get(), self.label.get())
    }
}

fn open(cx: &mut TestAppContext, cached: bool) -> (crate::WindowHandle<Root>, Counts) {
    let counts = Counts {
        root: Rc::default(),
        panel: Rc::default(),
        label: Rc::default(),
    };
    let (root, panel, label) = (counts.root.clone(), counts.panel.clone(), counts.label.clone());
    let window = cx.open_window(size(px(800.), px(600.)), move |_, cx| {
        let label = cx.new(|_| Label {
            renders: label,
            text: "9s",
        });
        let panel = cx.new(|_| Panel {
            renders: panel,
            label,
        });
        Root {
            renders: root,
            panel,
            cached,
        }
    });
    cx.run_until_parked();
    (window, counts)
}

fn notify_label(window: &crate::WindowHandle<Root>, cx: &mut TestAppContext, text: &'static str) {
    window
        .update(cx, |root, _, cx| {
            let label = root.panel.read(cx).label.clone();
            label.update(cx, |label, cx| {
                label.text = text;
                cx.notify();
            });
        })
        .unwrap();
    cx.run_until_parked();
}

fn label_bounds(window: &crate::WindowHandle<Root>, cx: &mut TestAppContext) -> crate::Bounds<crate::Pixels> {
    window
        .update(cx, |_, window, _| {
            window.rendered_frame.debug_bounds.get("LABEL").copied()
        })
        .unwrap()
        .expect("isolated label is drawn")
}

#[gpui::test]
fn notifying_an_isolated_view_redraws_only_that_view(cx: &mut TestAppContext) {
    let (window, counts) = open(cx, false);
    assert_eq!(counts.get(), (1, 1, 1));
    let before = label_bounds(&window, cx);
    assert_eq!(before.origin, point(px(30.), px(20.)));
    assert_eq!(before.size, size(px(40.), px(10.)));

    notify_label(&window, cx, "10s");
    assert_eq!(counts.get(), (1, 1, 2), "root and panel are kept");
    assert_eq!(label_bounds(&window, cx), before);

    // A kept frame records ranges a later kept frame replays again.
    notify_label(&window, cx, "11s");
    assert_eq!(counts.get(), (1, 1, 3));
    assert_eq!(label_bounds(&window, cx), before);
}

#[gpui::test]
fn a_parent_redraw_draws_the_isolated_view_again(cx: &mut TestAppContext) {
    let (window, counts) = open(cx, false);
    window.update(cx, |_, _, cx| cx.notify()).unwrap();
    cx.run_until_parked();
    assert_eq!(counts.get(), (2, 2, 2));
    notify_label(&window, cx, "12s");
    assert_eq!(counts.get(), (2, 2, 3));
}

#[gpui::test]
fn an_isolated_view_in_a_reused_cached_view_is_redrawn_alone(cx: &mut TestAppContext) {
    let (window, counts) = open(cx, true);
    // The root redraws; the cached panel and the label inside it are replayed.
    window.update(cx, |_, _, cx| cx.notify()).unwrap();
    cx.run_until_parked();
    assert_eq!(counts.get(), (2, 1, 1));
    let before = label_bounds(&window, cx);
    notify_label(&window, cx, "13s");
    assert_eq!(counts.get(), (2, 1, 2));
    assert_eq!(label_bounds(&window, cx), before);
}
