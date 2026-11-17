//! Отложенная фаза спаннера: пробы живут во вложенной раскладке, финальные
//! элементы создаются заново при окончательном ограничении ширины.
//!
//! `Adopt` instead paints the accepted measure tree and its existing layout directly.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, Pixels, RetainedLayout, Size, Window, point,
    px, size,
};
use std::{cell::Cell, cell::RefCell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpannerPass {
    Measure,
    Final,
}

type Build = dyn Fn(Size<AvailableSpace>, SpannerPass, &mut Window, &mut App) -> AnyElement;
type Intrinsic = dyn Fn(Size<AvailableSpace>, &mut Window, &mut App) -> f32;

/// The measure build of a spanner flow that prepaint may paint as it is.
///
/// Such a measure is built with the document's own paint collectors, in the
/// layout context of the flow's place and in its own retained engine. The
/// flow has nothing inside whose paint order or place its build time decides
/// (`spanners::adopt::adoptable`: no positioned box, no stacking context, no
/// fragmentation recorder), so the measure is the tree the final build would
/// make (Blink propagates a child's out-of-flow candidates and paint order
/// only when it takes its layout result,
/// `BoxFragmentBuilder::PropagateFromLayoutResult`).
pub(crate) struct Adopt {
    pub(crate) paint: Rc<crate::layout::cx::deferred::Deferred>,
}

/// A measure build kept for prepaint, by its resolved space.
type Kept = (Size<AvailableSpace>, AnyElement, RetainedLayout);

pub(crate) struct SpannerFlow {
    pub(crate) build: Rc<Build>,
    pub(crate) intrinsic: Rc<Intrinsic>,
    child: Option<AnyElement>,
    definite_height: Rc<Cell<Option<Pixels>>>,
    measured: Rc<MeasureMemo>,
    adopt: Option<Rc<Adopt>>,
    kept: Rc<RefCell<Option<Kept>>>,
    /// The engine of the adopted measure: prepaint and paint run in it.
    engine: Option<RetainedLayout>,
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
        build: impl Fn(Size<AvailableSpace>, SpannerPass, &mut Window, &mut App) -> AnyElement + 'static,
        intrinsic: impl Fn(Size<AvailableSpace>, &mut Window, &mut App) -> f32 + 'static,
        adopt: Option<Adopt>,
    ) -> AnyElement {
        Self {
            build: Rc::new(build),
            intrinsic: Rc::new(intrinsic),
            child: None,
            definite_height: Default::default(),
            measured: Default::default(),
            adopt: adopt.map(Rc::new),
            kept: Default::default(),
            engine: None,
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
        let measured = self.measured.clone();
        let adopt = self.adopt.clone();
        let kept = self.kept.clone();
        let context = adopt.is_some().then(|| window.element_layout_context());
        let mut style: gpui::Style = gpui::Style::default();
        // Анонимный поток сохраняет определённую высоту своего CSS-хоста;
        // при auto-высоте циклический процент остаётся неопределённым.
        style.size.height = gpui::relative(1.).into();
        let id = window.request_measured_layout_with_physical_baselines(
            style,
            move |known, available, window, cx| {
                definite_height.set(known.height);
                let height = known
                    .height
                    .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);
                let width = known
                    .width
                    .map(f32::from)
                    .unwrap_or_else(|| match available.width {
                        AvailableSpace::Definite(w) => f32::from(w),
                        other => window.with_nested_layout(|window| {
                            intrinsic(size(other, height), window, cx)
                        }),
                    });
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
                let mut engine = RetainedLayout::default();
                let held = adopt.as_ref().map(|a| a.paint.held_paint_last.get());
                let (child, content) = window.with_retained_layout(&mut engine, |window| {
                    let mut measure = |window: &mut Window| {
                        let mut child = build(available, SpannerPass::Measure, window, cx);
                        let mut snapshot = child.snapshot_layout_measurement(window, cx);
                        let content = snapshot.measure_physical_baselines(available, window, cx);
                        (child, content)
                    };
                    match &context {
                        Some(context) => window.with_element_layout_context(context, measure),
                        None => measure(window),
                    }
                });
                // A root-flow record asked for a wrapper inside: build again.
                let unregistered = adopt
                    .as_ref()
                    .zip(held)
                    .is_some_and(|(a, before)| a.paint.held_paint_last.get() == before);
                let keep = unregistered.then(|| (available, child, engine));
                let scheduled = keep.is_some() && kept.borrow().is_none();
                kept.replace(keep);
                // The measure build lives for its frame only: what prepaint
                // did not adopt is dropped when the frame ends, so no build
                // outlives it inside this closure (the retained engine keeps
                // the closure across frames).
                if scheduled {
                    let kept = kept.clone();
                    window.defer(cx, move |_, _| drop(kept.take()));
                }
                measured.set(Some((available, content.clone())));
                content
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
        let kept = self.kept.take().filter(|(space, ..)| *space == available);
        let Some((_, mut child, mut engine)) = kept else {
            self.engine = None;
            let mut child = (self.build)(available, SpannerPass::Final, window, cx);
            child.layout_as_root_at(origin, available, window, cx);
            child.prepaint_at(point(px(0.), px(0.)), window, cx);
            self.child = Some(child);
            return;
        };
        crate::work::add!(SpannerMeasureAdoptions, 1);
        window.with_retained_layout(&mut engine, |window| {
            child.layout_as_root_at(origin, available, window, cx);
            child.prepaint_at(point(px(0.), px(0.)), window, cx);
        });
        self.engine = Some(engine);
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
        // The flow's tree and its engine serve this frame's paint only: every
        // frame prepaints before it paints, so a retained element keeps no
        // built tree between frames (`reuse::FlowCx` keeps recorded flows).
        let Some(mut child) = self.child.take() else {
            return;
        };
        match self.engine.take() {
            Some(mut engine) => {
                window.with_retained_layout(&mut engine, |window| child.paint(window, cx))
            }
            None => child.paint(window, cx),
        }
    }
}

#[cfg(all(test, feature = "work-counters"))]
#[path = "element_tests.rs"]
mod tests;
