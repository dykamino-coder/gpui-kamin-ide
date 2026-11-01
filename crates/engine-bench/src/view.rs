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
    pub(super) viewport_override: Option<(u32, u32)>,
    pub(super) phases: Rc<RefCell<Phases>>,
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
                window.viewport_size().width.into(),
                window.viewport_size().height.into(),
            ),
            text,
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        if let Some((width, height)) = self.viewport_override {
            opts.viewport = (width as f32, height as f32);
        }
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

/// Каждый следующий замер меняет CSS viewport и требует нового кадра документа.
pub(super) fn relayout_viewport(base: (u32, u32), previous: Option<(u32, u32)>) -> (u32, u32) {
    let changed = (if base.0 == 1 { 2 } else { base.0 - 1 }, base.1);
    if previous == Some(changed) {
        base
    } else {
        changed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relayout_changes_each_frame_and_stays_inside_valid_viewports() {
        for width in [1, 800, 8192] {
            let base = (width, 600);
            let first = relayout_viewport(base, None);
            assert_ne!(first, base);
            assert!((1..=8192).contains(&first.0));
            let second = relayout_viewport(base, Some(first));
            assert_eq!(second, base);
            assert_eq!(relayout_viewport(base, Some(second)), first);
        }
    }
}
