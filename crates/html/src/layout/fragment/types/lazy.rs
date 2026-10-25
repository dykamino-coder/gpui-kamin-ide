//! Отложенные копии фрагментов: невыбранная планом колонка не строит поддерево.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, AvailableSpace, LayoutId, LayoutMeasurement, Pixels, Size, Window};
use std::{cell::RefCell, rc::Rc};

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

    pub(crate) fn measurement(&self, window: &mut Window, cx: &mut App) -> FragmentMeasurement {
        let ready = self
            .element
            .borrow_mut()
            .as_mut()
            .map(|el| el.snapshot_layout_measurement(window, cx));
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

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        self.prepare();
        (
            self.0
                .element
                .borrow_mut()
                .as_mut()
                .unwrap()
                .request_layout(window, cx),
            (),
        )
    }

    fn prepaint(&mut self, _: gpui::Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        self.0
            .element
            .borrow_mut()
            .as_mut()
            .unwrap()
            .prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: gpui::Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0
            .element
            .borrow_mut()
            .as_mut()
            .unwrap()
            .paint(window, cx);
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
    fn prepare(&mut self, window: &mut Window, cx: &mut App) -> &mut LayoutMeasurement {
        if self.ready.is_none() {
            let build = self.build.as_ref().expect("fragment recipe");
            // Родительская раскладка держит основной Taffy. Измерение получает
            // отдельный движок; его layout-id не попадают в дерево для paint.
            self.ready = Some(
                window.with_nested_layout(|window| build().snapshot_layout_measurement(window, cx)),
            );
        }
        self.ready.as_mut().unwrap()
    }

    pub(crate) fn measure(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        self.prepare(window, cx).measure(available, window, cx)
    }

    pub(crate) fn measure_slice(
        &mut self,
        available: Size<AvailableSpace>,
        from: Pixels,
        height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        self.prepare(window, cx)
            .measure_slice(available, from, height, window, cx)
    }
}
