//! Одно дерево GPUI; адаптер наблюдает lifecycle, не добавляя узлов раскладки.

use gpui::{
    AnyElement, App, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, ParentElement, Pixels, Render, Styled, Window, div, frame_perf, px, rgb,
};
use kamin_html::{Document, RenderOpts};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::Ordering::Relaxed,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct Scope {
    pub(super) wall: Duration,
    pub(super) layout: Duration,
}

impl Scope {
    fn start() -> (Instant, u32) {
        (Instant::now(), frame_perf::TAFFY_US.load(Relaxed))
    }

    fn finish(&mut self, (started, before): (Instant, u32)) {
        self.layout += Duration::from_micros(u64::from(
            frame_perf::TAFFY_US.load(Relaxed).wrapping_sub(before),
        ));
        self.wall += started.elapsed();
    }
}

#[derive(Default)]
pub(super) struct Phases {
    pub(super) build: Scope,
    pub(super) request_layout: Scope,
    pub(super) prepaint: Scope,
    pub(super) paint: Scope,
    pub(super) renders: usize,
}

pub(super) struct View {
    pub(super) doc: Document,
    pub(super) phases: Rc<RefCell<Phases>>,
}

impl Render for View {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let started = Scope::start();
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
        phases.build.finish(started);
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
        let started = Scope::start();
        let id = self.root.request_layout(window, cx);
        self.phases.borrow_mut().request_layout.finish(started);
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
        let started = Scope::start();
        self.root.prepaint(window, cx);
        self.phases.borrow_mut().prepaint.finish(started);
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
        let started = Scope::start();
        self.root.paint(window, cx);
        self.phases.borrow_mut().paint.finish(started);
    }
}
