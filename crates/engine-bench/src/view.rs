//! Одно дерево GPUI; адаптер наблюдает lifecycle, не добавляя узлов раскладки.

use gpui::{
    AnyElement, App, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, ParentElement, Pixels, Render, Styled, Window, div, px, rgb,
};
use kamin_html::{Document, RenderOpts};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct Phases {
    pub(super) build: Duration,
    pub(super) request_layout: Duration,
    pub(super) prepaint: Duration,
    pub(super) paint: Duration,
    pub(super) renders: usize,
}

pub(super) struct View {
    pub(super) doc: Document,
    pub(super) phases: Rc<RefCell<Phases>>,
}

impl Render for View {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let started = Instant::now();
        let mut text = window.text_style();
        text.color = rgb(0).into();
        text.font_family = "Times New Roman".into();
        text.font_size = px(16.).into();
        let mut opts = RenderOpts {
            viewport: (
                window.viewport_size().width.into(),
                window.viewport_size().height.into(),
            ),
            text,
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.deferred.freeze_scrollbars = true;
        let root = div()
            .w(px(opts.viewport.0))
            .h(px(opts.viewport.1))
            .bg(rgb(0xffffff))
            .text_size(px(16.))
            .font_family(opts.text.font_family.clone())
            .children(self.doc.render(&opts))
            .into_any_element();
        let mut phases = self.phases.borrow_mut();
        phases.build += started.elapsed();
        phases.renders += 1;
        Timed {
            root,
            phases: self.phases.clone(),
        }
    }
}

struct Timed {
    root: AnyElement,
    phases: Rc<RefCell<Phases>>,
}

impl IntoElement for Timed {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Timed {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let started = Instant::now();
        let id = self.root.request_layout(window, cx);
        self.phases.borrow_mut().request_layout += started.elapsed();
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let started = Instant::now();
        self.root.prepaint(window, cx);
        self.phases.borrow_mut().prepaint += started.elapsed();
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let started = Instant::now();
        self.root.paint(window, cx);
        self.phases.borrow_mut().paint += started.elapsed();
    }
}
