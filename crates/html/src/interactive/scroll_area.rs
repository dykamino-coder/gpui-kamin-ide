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
        }
    }
}

impl ScrollArea {
    pub(crate) fn with_style(mut self, style: crate::style::computed::Computed) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn with_tree(mut self, e: &crate::dom::Element) -> Self {
        if self.style.scroll.colors.flatten().is_some() {
            self.tree = Some(Rc::new(e.clone()));
        }
        self
    }
}

#[derive(Clone, Default)]
struct Persistent {
    handle: gpui::ScrollHandle,
    extent: super::scroll_extent::Frame,
}

impl Element for ScrollArea {
    type RequestLayoutState = (AnyElement, gpui::ScrollHandle, super::scroll_extent::Frame);
    type PrepaintState = ();

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
            let (layout_id, el) = state.extent.within(self.tree.as_deref(), || {
                let mut el = build(&state.handle, h, v);
                let layout_id = el.request_layout(window, cx);
                (layout_id, el)
            });
            (
                (layout_id, (el, state.handle.clone(), state.extent.clone())),
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
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child
            .2
            .within(self.tree.as_deref(), || child.0.prepaint(window, cx));
        if let Some(tree) = self.tree.as_deref()
            && child.2.settle(tree, &self.style, &child.1)
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
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.0.paint(window, cx);
        crate::interactive::scrollbars::paint(&self.style, &child.1, bounds, window);
    }
}

impl IntoElement for ScrollArea {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
