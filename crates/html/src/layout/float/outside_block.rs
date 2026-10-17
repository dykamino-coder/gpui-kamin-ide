//! Separate a float's physical size from its contribution to normal block sizing.

use crate::hooks::{IntoHooked, Wrap};
use crate::{
    dom::Element as DomElement,
    render::RenderOpts,
    style::computed::{Align, Computed},
};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, MeasuredContent, Pixels, Size, Style,
    Window, point, px, size,
};
use std::{cell::Cell, rc::Rc};

pub(crate) fn render(e: &DomElement, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut inner = e.clone();
    inner.marks.outside_block = false;
    let child = crate::render::element(&inner, inherited, opts);
    OutsideBlock {
        child,
        root: None,
        physical: Default::default(),
        block_rl: inherited.vertical_rl == Some(true),
        align: e.style.align_self,
    }
    .hooked()
}

struct OutsideBlock {
    child: AnyElement,
    root: Option<LayoutId>,
    physical: Rc<Cell<Size<Pixels>>>,
    block_rl: bool,
    align: Option<Align>,
}

impl Wrap for OutsideBlock {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let root = self.child.request_layout(window, cx);
        self.root = Some(root);
        let mut snapshot = window.snapshot_layout_measurement(root);
        let physical = self.physical.clone();
        // WM4 7.3: an indefinite containing inline axis falls back to the ICB.
        let fallback = window.viewport_size().width;
        let style = Style {
            size: size(px(0.0).into(), gpui::Length::Auto),
            flex_shrink: 0.0,
            align_self: match self.align {
                Some(Align::Start) => Some(gpui::AlignSelf::FlexStart),
                Some(Align::End) => Some(gpui::AlignSelf::FlexEnd),
                Some(Align::Center) => Some(gpui::AlignSelf::Center),
                _ => None,
            },
            ..Style::default()
        };
        window.request_measured_layout_with_physical_baselines(
            style,
            move |known, available, window, cx| {
                let height = known
                    .height
                    .map_or(available.height, AvailableSpace::Definite);
                let min = snapshot
                    .measure_physical_baselines(
                        size(AvailableSpace::MinContent, height),
                        window,
                        cx,
                    )
                    .size
                    .width;
                let max = snapshot
                    .measure_physical_baselines(
                        size(AvailableSpace::MaxContent, height),
                        window,
                        cx,
                    )
                    .size
                    .width;
                let width = min.max(max.min(fallback));
                let measured = snapshot.measure_physical_baselines(
                    size(AvailableSpace::Definite(width), height),
                    window,
                    cx,
                );
                physical.set(measured.size);
                MeasuredContent {
                    size: size(px(0.0), measured.size.height),
                    ..measured
                }
            },
        )
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let root = self.root.expect("float requested layout");
        let physical = self.physical.get();
        window.compute_layout(
            root,
            size(
                AvailableSpace::Definite(physical.width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            cx,
        );
        let mut origin = bounds.origin;
        if self.block_rl {
            origin.x -= window.layout_size_unrounded(root).width;
        }
        window.set_layout_root_origin(root, origin);
        self.child.prepaint_at(point(px(0.0), px(0.0)), window, cx);
    }
}
