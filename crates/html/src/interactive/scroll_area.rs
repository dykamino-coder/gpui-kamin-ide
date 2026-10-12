//! Элемент `ScrollArea`.
// owner: A

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::rc::Rc;

/// Прокрутка содержимого — `overflow: auto` и `scroll`.
///
/// Раньше оба значения давали только обрезку: часть содержимого пропадала без
/// возможности до неё добраться. Прокрутка в GPUI требует РУЧКИ, живущей между
/// кадрами, — а память есть только у своего элемента. Сама лента и колесо мыши
/// уже реализованы в `div`, поэтому здесь только ручка и её хранение.
pub struct ScrollArea {
    pub(crate) id: ElementId,
    pub(crate) horizontal: bool,
    pub(crate) vertical: bool,
    pub(crate) style: crate::style::computed::Computed,
    tree: Option<Rc<crate::dom::Element>>,
    parent_extent: Option<super::scroll_extent::Frame>,
    pub(crate) build: Rc<dyn Fn(&gpui::ScrollHandle, bool, bool) -> AnyElement>,
}

impl ScrollArea {
    pub fn new(
        id: ElementId,
        horizontal: bool,
        vertical: bool,
        build: Rc<dyn Fn(&gpui::ScrollHandle, bool, bool) -> AnyElement>,
    ) -> Self {
        ScrollArea {
            id,
            horizontal,
            vertical,
            build,
            style: Default::default(),
            tree: None,
            parent_extent: None,
        }
    }
}

impl ScrollArea {
    pub(crate) fn with_style(mut self, style: crate::style::computed::Computed) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn with_tree(mut self, e: &crate::dom::Element) -> Self {
        self.parent_extent = super::scroll_extent::Frame::parent_for(e.node_id);
        self.tree = Some(Rc::new(e.clone()));
        self
    }
}

#[derive(Clone, Default)]
struct Persistent {
    handle: gpui::ScrollHandle,
    extent: super::scroll_extent::Frame,
    interaction: super::scrollbars::interaction::Interaction,
}

impl Element for ScrollArea {
    type RequestLayoutState = (
        AnyElement,
        gpui::ScrollHandle,
        super::scroll_extent::Frame,
        LayoutId,
        super::scrollbars::interaction::Interaction,
    );
    type PrepaintState = (Bounds<Pixels>, Vec<super::scrollbars::interaction::HitBar>);

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let build = self.build.clone();
        let (h, v) = (self.horizontal, self.vertical);
        let layout = |state: Persistent, window: &mut Window, cx: &mut App| {
            state.extent.set_handle(state.handle.clone());
            let (layout_id, el) = state.extent.within(self.tree.as_deref(), || {
                let mut el = build(&state.handle, h, v);
                let layout_id = el.request_layout(window, cx);
                (layout_id, el)
            });
            (
                (
                    layout_id,
                    (
                        el,
                        state.handle.clone(),
                        state.extent.clone(),
                        layout_id,
                        state.interaction.clone(),
                    ),
                ),
                state,
            )
        };
        match id {
            Some(global_id) => window
                .with_element_state::<Persistent, _>(global_id, |state, window| {
                    layout(state.unwrap_or_default(), window, cx)
                }),
            None => layout(Persistent::default(), window, cx).0,
        }
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Match CSS border painting: inset the unrounded box, not a rounded
        // border box. A relocated prepaint_at node must retain its new bounds.
        let exact = Bounds {
            origin: window.layout_origin_unrounded(child.3),
            size: window.layout_size_unrounded(child.3),
        };
        let half = gpui::px(0.5 / window.scale_factor().max(0.01) + 1e-4);
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= half;
        let paint_bounds = if near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom())
        {
            exact
        } else {
            bounds
        };
        if let (Some(parent), Some(tree)) = (&self.parent_extent, self.tree.as_deref()) {
            parent.record_border_box(tree.node_id, &self.style, paint_bounds);
        }
        child.2.set_decoration_clip(window.content_mask());
        child
            .2
            .within(self.tree.as_deref(), || child.0.prepaint(window, cx));
        if let Some(tree) = self.tree.as_deref()
            && child.2.settle(tree, &self.style, &child.1, paint_bounds)
        {
            // Invalidation during prepaint is ignored by GPUI. Rebuild after
            // this frame, once the native extent proxy has its measured size.
            let view = window.current_view_opt();
            window.on_next_frame(move |window, cx| {
                if let Some(view) = view {
                    cx.notify(view);
                }
                window.refresh();
            });
        }
        let hitbars = super::scrollbars::interaction::prepaint(
            &self.style,
            &child.1,
            child.2.bars(),
            child.2.range(),
            child.2.insets(),
            paint_bounds,
            window,
        );
        (paint_bounds, hitbars)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        paint_bounds: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.2.set_decoration_clip(window.content_mask());
        child.0.paint(window, cx);
        let alpha = child.4.paint(&paint_bounds.1, &child.1, window);
        crate::interactive::scrollbars::paint(
            &self.style,
            &child.1,
            child.2.bars(),
            child.2.range(),
            child.2.insets(),
            paint_bounds.0,
            alpha,
            window,
        );
    }
}

impl IntoElement for ScrollArea {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
