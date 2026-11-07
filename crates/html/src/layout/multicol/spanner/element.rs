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

type Build = dyn for<'a> Fn(
    Size<AvailableSpace>,
    SpannerPass,
    &'a mut Window,
    &'a mut App,
    &'a mut gpui::PaintStack,
) -> gpui::PaintFuture<'a, AnyElement>;
type Intrinsic = dyn for<'a> Fn(
    Size<AvailableSpace>,
    &'a mut Window,
    &'a mut App,
    &'a mut gpui::PaintStack,
) -> gpui::PaintFuture<'a, f32>;

pub(crate) struct SpannerFlow {
    pub(crate) build: Rc<Build>,
    pub(crate) intrinsic: Rc<Intrinsic>,
    child: Option<AnyElement>,
    definite_height: Rc<Cell<Option<Pixels>>>,
    measured: Rc<MeasureMemo>,
}

/// The last measure-pass result of this element, by its resolved space.
///
/// A `SpannerPass::Measure` build is isolated (fresh positioning and paint
/// collectors, no fragmentation recorder), so the content it measures is a
/// function of the available space alone, in the requester's text scope.
/// Taffy asks the same leaf several times per layout (sizing, then the
/// final pass); every repeat at an equal space would rebuild every row and
/// re-balance every column set (Blink keeps one `LayoutResult` per box and
/// constraint space for the same reason, `layout_result_caching.cc`).
type MeasureMemo = Cell<Option<(Size<AvailableSpace>, gpui::MeasuredContent)>>;

impl SpannerFlow {
    pub(crate) fn deferred(
        build: impl for<'a> Fn(
            Size<AvailableSpace>,
            SpannerPass,
            &'a mut Window,
            &'a mut App,
            &'a mut gpui::PaintStack,
        ) -> gpui::PaintFuture<'a, AnyElement>
        + 'static,
        intrinsic: impl for<'a> Fn(
            Size<AvailableSpace>,
            &'a mut Window,
            &'a mut App,
            &'a mut gpui::PaintStack,
        ) -> gpui::PaintFuture<'a, f32>
        + 'static,
    ) -> AnyElement {
        Self {
            build: Rc::new(build),
            intrinsic: Rc::new(intrinsic),
            child: None,
            definite_height: Default::default(),
            measured: Default::default(),
        }
        .hooked()
    }
}

impl Hooks for SpannerFlow {
    type Layout = LayoutId;
    type Prepaint = ();

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        _cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, LayoutId) {
        let build = self.build.clone();
        let intrinsic = self.intrinsic.clone();
        let definite_height = self.definite_height.clone();
        let measured = self.measured.clone();
        let mut style: gpui::Style = gpui::Style::default();
        // Анонимный поток сохраняет определённую высоту своего CSS-хоста;
        // при auto-высоте циклический процент остаётся неопределённым.
        style.size.height = gpui::relative(1.).into();
        let id = window.request_measured_layout_with_physical_baselines_async(
            style,
            async move |known,
                        available,
                        window: &mut Window,
                        cx: &mut App,
                        stack: &mut gpui::PaintStack| {
                definite_height.set(known.height);
                window
                    .with_nested_layout_async(async |window| {
                        let height = known
                            .height
                            .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);
                        let width = match known.width {
                            Some(width) => f32::from(width),
                            None => match available.width {
                                AvailableSpace::Definite(width) => f32::from(width),
                                other => intrinsic(size(other, height), window, cx, stack).await,
                            },
                        };
                        let available = size(AvailableSpace::Definite(px(width)), height);
                        let last = measured.take();
                        if let Some((space, content)) = last.as_ref()
                            && *space == available
                        {
                            crate::work::add!(SpannerMeasureReuses, 1);
                            let content = content.clone();
                            measured.set(last);
                            return content;
                        }
                        let mut child =
                            build(available, SpannerPass::Measure, window, cx, stack).await;
                        let mut snapshot = child
                            .snapshot_layout_measurement_async(window, cx, stack)
                            .await;
                        let content = snapshot
                            .measure_physical_baselines_async(available, window, cx, stack)
                            .await;
                        measured.set(Some((available, content.clone())));
                        content
                    })
                    .await
            },
        );
        (id, id)
    }

    async fn prepaint_async(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let origin = window.layout_origin_unrounded(*state);
        let bounds = window.layout_size_unrounded(*state);
        let available = size(
            AvailableSpace::Definite(bounds.width),
            self.definite_height
                .get()
                .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite),
        );
        let mut child = (self.build)(available, SpannerPass::Final, window, cx, stack).await;
        child
            .layout_as_root_at_async(origin, available, window, cx, stack)
            .await;
        child
            .prepaint_at_async(point(px(0.), px(0.)), window, cx, stack)
            .await;
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
        crate::hooks::paint_on_stack(self, _bounds, _state, _prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _state: &'a mut LayoutId,
        _prepaint: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            if let Some(child) = &mut self.child {
                child.paint_async(window, cx, stack).await;
            }
        }
    }
}

#[cfg(all(test, feature = "work-counters"))]
#[path = "element_tests.rs"]
mod tests;
