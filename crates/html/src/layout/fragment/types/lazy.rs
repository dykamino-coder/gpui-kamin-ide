//! Отложенные копии фрагментов: невыбранная планом колонка не строит поддерево.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, AvailableSpace, LayoutId, LayoutMeasurement, Pixels, Size, Window};
use std::{cell::RefCell, rc::Rc};

mod element;
use element::ElementGuard;

type Builder = Rc<dyn Fn() -> AnyElement>;

#[derive(Clone)]
pub(crate) struct FragmentHandle {
    build: Builder,
    element: Rc<RefCell<Option<AnyElement>>>,
}

impl FragmentHandle {
    pub(crate) fn new(build: impl Fn() -> AnyElement + 'static) -> Self {
        Self {
            build: Rc::new(build),
            element: Default::default(),
        }
    }

    pub(crate) fn element(&self) -> AnyElement {
        Deferred(self.clone()).hooked()
    }

    pub(crate) async fn measurement_async(
        &self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> FragmentMeasurement {
        let mut element = ElementGuard::take(&self.element);
        let ready = if let Some(el) = element.as_mut() {
            Some(
                el.snapshot_layout_measurement_async(window, cx, stack)
                    .await,
            )
        } else {
            None
        };
        FragmentMeasurement {
            ready,
            build: Some(self.build.clone()),
        }
    }
}

struct Deferred(FragmentHandle);

impl Deferred {
    // Кадры построителя завершаются до спуска request_layout в следующую копию.
    #[inline(never)]
    fn prepare(&mut self) {
        if self.0.element.borrow().is_none() {
            let element = (self.0.build)();
            *self.0.element.borrow_mut() = Some(element);
        }
    }
}

impl Hooks for Deferred {
    type Layout = ();
    type Prepaint = ();

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        self.prepare();
        let mut element = ElementGuard::take(&self.0.element);
        (
            element
                .as_mut()
                .unwrap()
                .request_layout_async(window, cx, stack)
                .await,
            (),
        )
    }

    async fn prepaint_async(
        &mut self,
        _: gpui::Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let mut element = ElementGuard::take(&self.0.element);
        element
            .as_mut()
            .unwrap()
            .prepaint_async(window, cx, stack)
            .await;
    }

    fn paint(
        &mut self,
        bounds: gpui::Bounds<Pixels>,
        layout: &mut (),
        prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, layout, prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: gpui::Bounds<Pixels>,
        _layout: &'a mut (),
        _prepaint: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            let mut element = ElementGuard::take(&self.0.element);
            element
                .as_mut()
                .unwrap()
                .paint_async(window, cx, stack)
                .await;
        }
    }
}

/// Копия измерительной раскладки создаётся только при обращении к фрагменту плана.
pub(crate) struct FragmentMeasurement {
    ready: Option<LayoutMeasurement>,
    build: Option<Builder>,
}

impl FragmentMeasurement {
    pub(crate) fn ready(measurement: LayoutMeasurement) -> Self {
        Self {
            ready: Some(measurement),
            build: None,
        }
    }

    #[inline(never)]
    async fn prepare_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> &mut LayoutMeasurement {
        if self.ready.is_none() {
            let build = self.build.as_ref().expect("fragment recipe");
            // Родительская раскладка держит основной Taffy. Измерение получает
            // отдельный движок; его layout-id не попадают в дерево для paint.
            self.ready = Some(
                window
                    .with_retained_layout_async(
                        &mut gpui::RetainedLayout::default(),
                        async |window| {
                            build()
                                .snapshot_layout_measurement_async(window, cx, stack)
                                .await
                        },
                    )
                    .await,
            );
        }
        self.ready.as_mut().unwrap()
    }

    pub(crate) async fn measure_async(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        self.prepare_async(window, cx, stack)
            .await
            .measure_async(available, window, cx, stack)
            .await
    }

    pub(crate) async fn measure_slice_async(
        &mut self,
        available: Size<AvailableSpace>,
        from: Pixels,
        height: Pixels,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        self.prepare_async(window, cx, stack)
            .await
            .measure_slice_async(available, from, height, window, cx, stack)
            .await
    }
}
