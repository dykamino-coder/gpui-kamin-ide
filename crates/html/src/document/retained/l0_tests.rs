//! Шаг документа до кадра (`prepared`): кадр не собирается внутри
//! `Window::draw`, раскладка шага ставит каждую коробку туда же, где её
//! ставит сборка в кадре, а чтение между кадрами не оставляет следов.

use super::tests::{frame, open};
use super::{Builder, FrameKey};
use crate::{Document, RenderOpts};
use gpui::{
    App, AvailableSpace, Bounds, Element, ElementId, ElementPlace, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, ParentElement, Pixels, Styled, TestAppContext, Window, div, point, px,
    size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub(super) const PAGE: &str = "<p style='margin:20px'>hello <b>world</b> and a longer line of text \
 that wraps somewhere in the middle of the box</p>\
 <div style='display:flex;gap:4px'><div style='flex:1'>a</div><div style='width:30px'>b</div></div>\
 <div style='display:grid;grid-template-columns:1fr 2fr'><div>c</div><div>d d d</div></div>\
 <table border=1><tr><td>e</td><td>f f</td></tr><tr><td colspan=2>g</td></tr></table>\
 <div><div style='float:left;width:40px;height:30px;margin:3px'></div>text beside a float \
 <span style='display:inline-block;width:20px;height:10px'></span> atom and more words to wrap</div>\
 <ol><li>one<li>two</ol><div style='columns:2'>x y z w</div>";

fn deep() -> String {
    let open = "<div style='padding:0 1px;border-left:1px solid red;margin-top:2px'>".repeat(150);
    format!("{open}<span>bottom</span>{}", "</div>".repeat(150))
}

fn opts() -> RenderOpts {
    let mut opts = RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (400., 300.),
        normal_line_height: 1.31,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.deferred.freeze_scrollbars = true;
    opts
}

/// Every box of a frame tree built by `doc`, laid out as the frame lays it out.
fn boxes(doc: &Document, window: &mut Window, cx: &mut App) -> Vec<Bounds<Pixels>> {
    let builder: Builder = doc.frame_builder(&opts(), &FrameKey::of(&opts()));
    let (mut tree, _) = builder.build(false, false);
    let id = tree.request_layout(window, cx);
    let space = size(AvailableSpace::Definite(px(400.)), AvailableSpace::Definite(px(300.)));
    window.compute_layout(id, space, cx);
    window.subtree_layout_bounds(id)
}

/// Builds of the frame so far, and those made inside `Window::draw`.
pub(super) fn builds(view: &gpui::Entity<super::tests::View>, cx: &mut gpui::VisualTestContext) -> (u64, u64) {
    cx.update(|_, app| {
        let doc = &view.read(app).doc;
        (doc.retained_frames().0, doc.builds_inside_draw())
    })
}

#[gpui::test]
fn frames_are_built_by_the_step_not_inside_draw(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, PAGE);
    // The first frame places the document: only then the step knows where.
    frame(cx);
    let (built, inside) = builds(&view, cx);
    frame(cx);
    cx.update(|_, app| view.update(app, |v, _| v.viewport = (500., 300.)));
    frame(cx);
    cx.update(|_, app| view.update(app, |v, _| v.viewport = (450., 280.)));
    frame(cx);
    let (built_after, inside_after) = builds(&view, cx);
    assert!(built_after >= built + 2, "the step built no frame: {built} -> {built_after}");
    assert_eq!(inside_after, inside, "frames were built inside draw");
}

fn same_boxes(html: &str, cx: &mut TestAppContext) {
    let doc = Document::builder(html, "").build().unwrap();
    let cx = cx.add_empty_window();
    let inside = Rc::new(RefCell::new(Vec::new()));
    let seen = inside.clone();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(300.)), |window, app| {
        *seen.borrow_mut() = boxes(&doc, window, app);
        div()
    });
    let place = ElementPlace::default();
    let outside = cx.update(|window, app| {
        window.with_layout_outside_draw(&place, app, |w, app| boxes(&doc, w, app))
    });
    let read =
        cx.update(|window, app| window.with_layout_read(&place, app, |w, app| boxes(&doc, w, app)));
    let inside = inside.borrow();
    assert!(inside.len() > 10, "too few boxes: {}", inside.len());
    assert_eq!(*inside, outside, "the step placed a box elsewhere");
    assert_eq!(*inside, read, "a read placed a box elsewhere");
}

#[gpui::test]
fn the_step_places_every_box_where_the_frame_does(cx: &mut TestAppContext) {
    same_boxes(PAGE, cx);
}

#[gpui::test]
fn the_step_places_every_box_of_a_deep_chain_where_the_frame_does(cx: &mut TestAppContext) {
    same_boxes(&deep(), cx);
}

#[gpui::test]
fn reads_between_frames_leave_no_elements_lines_or_states(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, PAGE);
    frame(cx);
    let (_, inside) = builds(&view, cx);
    let before = cx.update(|window, app| window.layout_footprint(app));
    for _ in 0..5 {
        cx.update(|window, app| {
            window.with_layout_read(&ElementPlace::default(), app, |w, app| {
                view.update(app, |v, cx| {
                    let kids = v.doc.render(&opts());
                    let mut tree = div().w(px(400.)).children(kids).into_any_element();
                    let space =
                        size(AvailableSpace::Definite(px(400.)), AvailableSpace::MinContent);
                    tree.layout_as_root(space, w, cx);
                })
            })
        });
    }
    let after = cx.update(|window, app| window.layout_footprint(app));
    assert_eq!(before, after, "reads left a trace between frames");
    cx.update(|_, app| view.update(app, |v, _| v.viewport = (500., 300.)));
    frame(cx);
    assert_eq!(builds(&view, cx).1, inside, "a frame after reads was built inside draw");
}

/// Counts its layouts in its element state.
struct Counter(Rc<Cell<u32>>);

impl IntoElement for Counter {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Counter {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("l0-counter".into()))
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let n = window.with_element_state::<u32, _>(id.unwrap(), |n, _| {
            let n = n.unwrap_or(0) + 1;
            (n, n)
        });
        self.0.set(n);
        (window.request_layout(gpui::Style::default(), [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

#[gpui::test]
fn a_frame_takes_the_element_state_a_read_moved(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let n = Rc::new(Cell::new(0));
    let space = size(px(100.), px(100.));
    cx.draw(point(px(0.), px(0.)), space, |_, _| Counter(n.clone()));
    assert_eq!(n.get(), 1);
    let set_aside = cx.update(|window, app| {
        window.with_layout_read(&ElementPlace::default(), app, |w, app| {
            let mut el = Counter(n.clone()).into_any_element();
            el.layout_as_root(space.map(AvailableSpace::Definite), w, app);
        });
        window.layout_footprint(app).2
    });
    assert_eq!(n.get(), 2, "the read did not see the frame's state");
    assert_eq!(set_aside, 1, "the read did not set its state aside");
    cx.draw(point(px(0.), px(0.)), space, |_, _| Counter(n.clone()));
    assert_eq!(n.get(), 3, "the frame lost the state the read moved");
    assert_eq!(cx.update(|window, app| window.layout_footprint(app).2), 0);
}
