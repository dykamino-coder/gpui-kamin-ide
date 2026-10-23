//! ClampHost: проба полной раскладки и применение её плана в текущем кадре.

use super::geometry::ClampGeometry;
use super::{ClampInput, ClampMeasure, ClampPlan};
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, MeasuredBox, Pixels, Size, Style, Window,
    point, px, size,
};
use std::{cell::RefCell, rc::Rc};

pub(crate) type Build = Rc<dyn Fn(ClampInput, Option<Pixels>, MeasuredBox, bool) -> AnyElement>;
type Cached = Rc<RefCell<Option<(Pixels, Rc<ClampPlan>, MeasuredBox)>>>;

pub(crate) struct ClampHost {
    pub(crate) key: u64,
    pub(crate) style: Style,
    pub(crate) geometry: ClampGeometry,
    pub(crate) build: Build,
    pub(crate) border_box: bool,
    pub(crate) paint: Rc<crate::layout::cx::deferred::Deferred>,
    plan: Cached,
    built: Option<AnyElement>,
}

impl ClampHost {
    pub(crate) fn new(
        key: u64,
        style: Style,
        geometry: ClampGeometry,
        build: Build,
        border_box: bool,
        paint: Rc<crate::layout::cx::deferred::Deferred>,
    ) -> Self {
        Self {
            key,
            style,
            geometry,
            build,
            border_box,
            paint,
            plan: Default::default(),
            built: None,
        }
    }
}

mod measure;
use measure::measure;

impl Hooks for ClampHost {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _: &mut App) -> (LayoutId, LayoutId) {
        let (key, border_box) = (self.key, self.border_box);
        let geometry = self.geometry.clone();
        let build = self.build.clone();
        let cache = self.plan.clone();
        let id = window.request_measured_layout_with_box(
            self.style.clone(),
            move |known, available, used_box, window, cx| {
                let edges = used_box.inset();
                let inset = size(edges.left + edges.right, edges.top + edges.bottom);
                let add = |space, edge| match space {
                    AvailableSpace::Definite(value) => AvailableSpace::Definite(value + edge),
                    other => other,
                };
                let available = size(
                    add(available.width, inset.width),
                    add(available.height, inset.height),
                );
                let (size, plan) = measure(
                    key, &geometry, &build, used_box, border_box, known, available, window, cx,
                );
                *cache.borrow_mut() = Some((size.width, plan, used_box));
                // Native leaf ожидает content-box и сам добавит разрешённые края.
                gpui::size(
                    (size.width - inset.width).max(px(0.)),
                    (size.height - inset.height).max(px(0.)),
                )
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
        let used_box = self
            .plan
            .borrow()
            .as_ref()
            .map(|(_, _, used)| *used)
            .expect("performed measured layout supplies the original containing block");
        let plan = self
            .plan
            .borrow()
            .as_ref()
            .filter(|(width, _, _)| *width == extent.width)
            .map(|(_, plan, _)| plan.clone())
            .unwrap_or_else(|| {
                measure(
                    self.key,
                    &self.geometry,
                    &self.build,
                    used_box,
                    self.border_box,
                    size(Some(extent.width), Some(extent.height)),
                    space,
                    window,
                    cx,
                )
                .1
            });
        let mut el = (self.build)(ClampInput::Apply(plan), Some(extent.width), used_box, true);
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
