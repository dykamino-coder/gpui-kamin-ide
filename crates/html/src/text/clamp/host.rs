//! ClampHost: проба полной раскладки и применение её плана в текущем кадре.

use super::cut_element::ClampGeometry;
use super::{ClampInput, ClampMeasure, ClampPlan};
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, Pixels, Size, Style, Window, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

pub(crate) type Build = Rc<dyn Fn(ClampInput, Option<Pixels>, bool) -> AnyElement>;
type Cached = Rc<RefCell<Option<(Pixels, Rc<ClampPlan>)>>>;

pub(crate) struct ClampHost {
    pub(crate) key: u64,
    pub(crate) style: Style,
    pub(crate) geometry: ClampGeometry,
    pub(crate) build: Build,
    pub(crate) content_top: f32,
    pub(crate) paint: Rc<crate::paint::context::PaintCx>,
    plan: Cached,
    built: Option<AnyElement>,
}

impl ClampHost {
    pub(crate) fn new(
        key: u64,
        style: Style,
        geometry: ClampGeometry,
        build: Build,
        content_top: f32,
        paint: Rc<crate::paint::context::PaintCx>,
    ) -> Self {
        Self {
            key,
            style,
            geometry,
            build,
            content_top,
            paint,
            plan: Default::default(),
            built: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn measure(
    key: u64,
    geometry: &ClampGeometry,
    build: &Build,
    content_top: f32,
    known: Size<Option<Pixels>>,
    available: Size<AvailableSpace>,
    window: &mut Window,
    cx: &mut App,
) -> (Size<Pixels>, Rc<ClampPlan>) {
    window.with_nested_layout(|window| {
        let records = ClampMeasure::default();
        let mut trial = build(ClampInput::Measure(records.clone()), known.width, false);
        let space = size(
            known
                .width
                .map_or(available.width, AvailableSpace::Definite),
            known
                .height
                .map_or(available.height, AvailableSpace::Definite),
        );
        trial.layout_as_root(space, window, cx);
        let batch = records.collect(key, point(px(0.), px(0.)), window);
        let mut plan = Rc::new(geometry.compute(batch.entries, &batch.rows, content_top, None));
        // Балансировка после выбора точки может увеличить высоту строк
        // (css-text-4 §5.4). Повторная проба строго уменьшает бюджет того же
        // абзаца: конечность не зависит от кадров и прежнего ClampState.
        // Размер берётся из применения плана, а не из n * line-height.
        loop {
            let applied_records = ClampMeasure::default();
            let mut applied = build(
                ClampInput::ApplyMeasured(plan.clone(), applied_records.clone()),
                known.width,
                false,
            );
            let measured = applied.layout_as_root(space, window, cx);
            let batch = applied_records.collect(key, point(px(0.), px(0.)), window);
            let next = geometry.compute(batch.entries, &batch.rows, content_top, None);
            if plan
                .para
                .zip(next.para)
                .is_some_and(|((s, k), (ns, nk))| s == ns && nk < k)
            {
                plan = Rc::new(next);
                continue;
            }
            return (
                size(
                    known.width.unwrap_or(measured.width),
                    known.height.unwrap_or(measured.height),
                ),
                plan,
            );
        }
    })
}

impl Hooks for ClampHost {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _: &mut App) -> (LayoutId, LayoutId) {
        let (key, top) = (self.key, self.content_top);
        let geometry = self.geometry.clone();
        let build = self.build.clone();
        let cache = self.plan.clone();
        let id = window.request_measured_layout(
            self.style.clone(),
            move |known, available, window, cx| {
                let (size, plan) =
                    measure(key, &geometry, &build, top, known, available, window, cx);
                *cache.borrow_mut() = Some((size.width, plan));
                size
            },
        );
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let extent = window.layout_size_unrounded(*id);
        let origin = window.layout_origin_unrounded(*id);
        let space = size(
            AvailableSpace::Definite(extent.width),
            AvailableSpace::Definite(extent.height),
        );
        // При двух заданных размерах taffy вправе не вызывать measure.
        let plan = self
            .plan
            .borrow()
            .as_ref()
            .filter(|(width, _)| *width == extent.width)
            .map(|(_, plan)| plan.clone())
            .unwrap_or_else(|| {
                measure(
                    self.key,
                    &self.geometry,
                    &self.build,
                    self.content_top,
                    size(Some(extent.width), Some(extent.height)),
                    space,
                    window,
                    cx,
                )
                .1
            });
        #[cfg(feature = "wpt-harness")]
        {
            let actual = measure(
                self.key,
                &self.geometry,
                &self.build,
                self.content_top,
                size(Some(extent.width), Some(extent.height)),
                space,
                window,
                cx,
            )
            .1;
            super::ClampCut::check(&plan, &actual);
        }
        let mut el = (self.build)(ClampInput::Apply(plan), Some(extent.width), true);
        el.layout_as_root_at(origin, space, window, cx);
        el.prepaint_at(point(px(0.), px(0.)), window, cx);
        self.built = Some(el);
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::paint::decorations::outline_phase::flow(
            Some(&self.paint),
            window,
            cx,
            |window, cx| {
                gpui::paint_reopen(|| {
                    if let Some(el) = &mut self.built {
                        el.paint(window, cx);
                    }
                });
            },
        );
    }
}
