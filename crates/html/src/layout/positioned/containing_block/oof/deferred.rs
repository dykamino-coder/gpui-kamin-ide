//! Внепоточные потомки отложенного построителя в слое исходного CB.

use super::OofHost;
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, IntoElement, LayoutId, Pixels, Position, Style,
    Window, div, point, prelude::*, px, relative, size,
};

pub(super) struct DeferredOof {
    pending: OofHost,
    built: Option<AnyElement>,
}

impl DeferredOof {
    pub(super) fn new(pending: OofHost) -> Self {
        Self {
            pending,
            built: None,
        }
    }
}

impl Hooks for DeferredOof {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let mut style = Style {
            position: Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.size = size(relative(1.).into(), relative(1.).into());
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let pending = self.pending.clone();
        if pending.is_empty() {
            return;
        }
        let extent = window.layout_size_unrounded(*id);
        let origin = window.layout_origin_unrounded(*id);
        let mut el = div()
            .relative()
            .w(extent.width)
            .h(extent.height)
            .children(pending.place(super::super::layers::icb_place))
            .into_any_element();
        el.layout_as_root_at(
            origin,
            size(
                AvailableSpace::Definite(extent.width),
                AvailableSpace::Definite(extent.height),
            ),
            window,
            cx,
        );
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
        gpui::paint_reopen(|| {
            if let Some(el) = &mut self.built {
                el.paint(window, cx);
            }
        });
    }
}
