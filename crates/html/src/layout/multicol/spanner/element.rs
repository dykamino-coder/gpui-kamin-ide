//! Отложенная фаза спаннера: пробы живут во вложенной раскладке, финальные
//! элементы создаются заново при окончательном ограничении ширины.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, Pixels, Size, Window, point, px, size,
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpannerPass {
    Measure,
    Final,
}

type Build = dyn Fn(Size<AvailableSpace>, SpannerPass, &mut Window, &mut App) -> AnyElement;
type Intrinsic = dyn Fn(Size<AvailableSpace>, &mut Window, &mut App) -> f32;

pub(crate) struct SpannerFlow {
    pub(crate) build: Rc<Build>,
    pub(crate) intrinsic: Rc<Intrinsic>,
    child: Option<AnyElement>,
    definite_height: Rc<Cell<Option<Pixels>>>,
}

impl SpannerFlow {
    pub(crate) fn deferred(
        build: impl Fn(Size<AvailableSpace>, SpannerPass, &mut Window, &mut App) -> AnyElement + 'static,
        intrinsic: impl Fn(Size<AvailableSpace>, &mut Window, &mut App) -> f32 + 'static,
    ) -> AnyElement {
        Self {
            build: Rc::new(build),
            intrinsic: Rc::new(intrinsic),
            child: None,
            definite_height: Default::default(),
        }
        .hooked()
    }
}

impl Hooks for SpannerFlow {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, LayoutId) {
        let build = self.build.clone();
        let intrinsic = self.intrinsic.clone();
        let definite_height = self.definite_height.clone();
        let mut style: gpui::Style = gpui::Style::default();
        // Анонимный поток сохраняет определённую высоту своего CSS-хоста;
        // при auto-высоте циклический процент остаётся неопределённым.
        style.size.height = gpui::relative(1.).into();
        let id = window.request_measured_layout_with_physical_baselines(
            style,
            move |known, available, window, cx| {
                definite_height.set(known.height);
                window.with_nested_layout(|window| {
                    let height = known
                        .height
                        .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);
                    let width =
                        known
                            .width
                            .map(f32::from)
                            .unwrap_or_else(|| match available.width {
                                AvailableSpace::Definite(w) => f32::from(w),
                                other => intrinsic(size(other, height), window, cx),
                            });
                    let available = size(AvailableSpace::Definite(px(width)), height);
                    let mut child = build(available, SpannerPass::Measure, window, cx);
                    let mut snapshot = child.snapshot_layout_measurement(window, cx);
                    snapshot.measure_physical_baselines(available, window, cx)
                })
            },
        );
        (id, id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = window.layout_origin_unrounded(*state);
        let bounds = window.layout_size_unrounded(*state);
        let available = size(
            AvailableSpace::Definite(bounds.width),
            self.definite_height
                .get()
                .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite),
        );
        let mut child = (self.build)(available, SpannerPass::Final, window, cx);
        child.layout_as_root_at(origin, available, window, cx);
        child.prepaint_at(point(px(0.), px(0.)), window, cx);
        self.child = Some(child);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = &mut self.child {
            child.paint(window, cx);
        }
    }
}
