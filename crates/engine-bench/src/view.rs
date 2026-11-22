//! Одно дерево GPUI; адаптер наблюдает lifecycle, не добавляя узлов раскладки.

use gpui::{
    AnyElement, App, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Render, Window, px, rgb,
};
use kamin_html::{Document, RenderOpts, work::Clock};
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
    /// Renders inside the draw of the previous render: GPUI's layout retry
    /// (`Window::update_layout_input`) discards an attempt and renders the
    /// root again in the same draw, for example after a container query
    /// input changed. No new frame was requested for them.
    pub(super) layout_retries: usize,
    /// The draw of the latest render.
    pass: Option<gpui::PaintPassId>,
    pub(super) observer_setup: Duration,
    pub(super) requested: Option<Instant>,
    pub(super) request_to_render: Option<Duration>,
    request_finished: Option<Clock>,
}

impl Phases {
    /// Draws that rendered the root: a layout retry renders again inside
    /// its draw and is the same frame, timed whole.
    pub(super) fn frame_renders(&self) -> usize {
        self.renders - self.layout_retries
    }

    pub(super) fn requested(observer_setup: Duration) -> Self {
        Self {
            observer_setup,
            requested: Some(Instant::now()),
            ..Default::default()
        }
    }
}

pub(super) struct View {
    pub(super) doc: Document,
    pub(super) phases: Rc<RefCell<Phases>>,
    pub(super) viewport: Option<(u32, u32)>,
    /// Шаг документа перед каждым кадром окна (`step::register`).
    pub(super) step: Option<gpui::Subscription>,
    /// Опции, с которыми шаг этого кадра подготовил документ: кадр берёт их
    /// же, а не строит вторые (каждые опции — свои контексты раскладки).
    pub(super) step_opts: Option<RenderOpts>,
    /// Root scroll offset the next fresh document opens at (`scroll_fresh`).
    pub(super) fresh_scroll: Option<(f32, f32)>,
}

impl View {
    /// Опции кадра документа: те же у шага и у кадра.
    pub(super) fn opts(&self, window: &Window) -> RenderOpts {
        let mut text = window.text_style();
        text.color = rgb(0).into();
        text.font_family = "Times New Roman".into();
        text.font_size = px(16.).into();
        let mut opts = RenderOpts {
            viewport: self.viewport.map_or_else(
                || {
                    (
                        window.viewport_size().width.into(),
                        window.viewport_size().height.into(),
                    )
                },
                |(w, h)| (w as f32, h as f32),
            ),
            text,
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.deferred.freeze_scrollbars = true;
        opts
    }
}

impl Render for View {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        kamin_html::work::phase!(_work, BuildElements);
        let started = Clock::now();
        let mut phases = self.phases.borrow_mut();
        // Шаг документа (`step`) уже отметил начало кадра.
        if phases.request_to_render.is_none() {
            let rendered = Instant::now();
            phases.request_to_render = phases
                .requested
                .map(|request| rendered.duration_since(request));
        }
        drop(phases);
        let opts = self.step_opts.take().unwrap_or_else(|| self.opts(window));
        // Коробка области просмотра принадлежит документу: неизменный кадр
        // он повторяет без сборки и раскладки (`Document::render_viewport`).
        let root = self.doc.render_viewport(&opts);
        let mut phases = self.phases.borrow_mut();
        phases.build += started.elapsed();
        phases.renders += 1;
        // DevTools `why`: each render in the order of the frame requests (`--why`).
        gpui::dirty_sources::trace::mark("render");
        let pass = window.paint_pass_id();
        if phases.pass.as_ref() == Some(&pass) {
            phases.layout_retries += 1;
        }
        phases.pass = Some(pass);
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
        let started = Clock::now();
        kamin_html::work::switch!(LayoutRequest);
        let id = self.root.request_layout(window, cx);
        let mut phases = self.phases.borrow_mut();
        phases.request_layout += started.elapsed();
        phases.request_finished = Some(Clock::now());
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
        let started = Clock::now();
        {
            let mut phases = self.phases.borrow_mut();
            // Между callback выполняются root layout и обвязка GPUI; вложенные
            // layout внутри prepaint остаются в prepaint, не считаясь повторно.
            // Раскладка шага до кадра — часть той же фазы.
            let step = phases.layout.take().unwrap_or_default();
            phases.layout = phases
                .request_finished
                .take()
                .map(|end| started.duration_since(end) + step);
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
        let started = Clock::now();
        kamin_html::work::switch!(Paint);
        self.root.paint(window, cx);
        kamin_html::work::switch!(RasterGpuReadback);
        self.phases.borrow_mut().paint += started.elapsed();
    }
}
