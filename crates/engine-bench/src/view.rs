//! Одно дерево GPUI; адаптер наблюдает lifecycle, не добавляя узлов раскладки.

use gpui::{
    AnyElement, App, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Render, Window, px, rgb,
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
    pub(super) layout: Option<Duration>,
    pub(super) prepaint: Duration,
    pub(super) paint: Duration,
    pub(super) renders: usize,
    request_finished: Option<Instant>,
}

pub(super) struct View {
    pub(super) doc: Document,
    pub(super) phases: Rc<RefCell<Phases>>,
    /// Ширина области просмотра вместо ширины окна (сценарий `--edit resize`).
    pub(super) width: Option<f32>,
}

impl Render for View {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        kamin_html::work::phase!(_work, BuildElements);
        let started = Instant::now();
        let mut text = window.text_style();
        text.color = rgb(0).into();
        text.font_family = "Times New Roman".into();
        text.font_size = px(16.).into();
        let mut opts = RenderOpts {
            viewport: (
                self.width
                    .unwrap_or_else(|| window.viewport_size().width.into()),
                window.viewport_size().height.into(),
            ),
            text,
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.deferred.freeze_scrollbars = true;
        // Коробка области просмотра принадлежит документу: неизменный кадр
        // он повторяет без сборки и раскладки (`Document::render_viewport`).
        let root = self.doc.render_viewport(&opts);
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
        kamin_html::work::switch!(LayoutRequest);
        let id = self.root.request_layout(window, cx);
        let mut phases = self.phases.borrow_mut();
        phases.request_layout += started.elapsed();
        phases.request_finished = Some(Instant::now());
        kamin_html::work::switch!(Layout);
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
        {
            let mut phases = self.phases.borrow_mut();
            // Между callback выполняются root layout и обвязка GPUI; вложенные
            // layout внутри prepaint остаются в prepaint, не считаясь повторно.
            phases.layout = phases
                .request_finished
                .take()
                .map(|end| started.duration_since(end));
        }
        kamin_html::work::switch!(Prepaint);
        self.root.prepaint(window, cx);
        kamin_html::work::switch!(PipelineOther);
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
        kamin_html::work::switch!(Paint);
        self.root.paint(window, cx);
        kamin_html::work::switch!(RasterGpuReadback);
        self.phases.borrow_mut().paint += started.elapsed();
    }
}
