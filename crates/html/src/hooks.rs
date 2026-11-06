//! Единственный адаптер движка к `gpui::Element`: обёртки и пробы реализуют [`Hooks`]
//! (типизированное состояние жизненного цикла) или [`Wrap`] (один потомок, методы
//! пересылки по умолчанию) и становятся элементами через [`Hooked`].
//!
//! `Element` в gpui требует от каждого реализатора id, место в исходниках, два
//! игнорируемых id-параметра на фазу и `IntoElement`. Обёртки движка id не носят,
//! поэтому эта рамка живёт здесь один раз. Элементы, хранящие состояние gpui между
//! кадрами (по `GlobalElementId`) — `Transition`, `Selectable`, `Resizable`,
//! `ScrollArea`, `Paragraph` — и `FilteredImage` (делегирует gpui `Img` с его id)
//! реализуют `gpui::Element` напрямую.

mod paint;
#[cfg(test)]
pub(crate) use paint::{paint_collect, paint_inline, paint_reopen};
mod painter;
mod traversal;
pub(crate) use paint::BareRecords;
pub(crate) use paint::{LineCapture, paint_capturing_async, replay_deferred};
pub(crate) use paint::{Replay, SlotScopes, presented_hit_scopes};
pub(crate) use paint::{
    SourceSlices, paint_source_node_async, paint_source_record_async, prepaint_source_record_async,
};
pub(crate) use paint::{paint_atomic, paint_last, paint_reopen_async, paint_root};
pub(crate) use painter::{PaintOnly, Painter};
pub(crate) use traversal::*;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

/// The three gpui phases with typed state and without element ids.
pub(crate) trait Hooks: 'static {
    fn paint_boundary(&self) -> bool {
        true
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        None
    }

    /// State from `request_layout`, handed to `prepaint` and `paint`.
    type Layout: 'static;
    /// State from `prepaint`, handed to `paint`.
    type Prepaint: 'static;

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout);

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint;

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    );
    fn paint_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        layout: &'a mut Self::Layout,
        prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        _stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(async move {
            self.paint(bounds, layout, prepaint, window, cx);
        })
    }
}

/// Синхронная точка входа для реализации Hooks с продолжением paint.
pub(crate) fn paint_on_stack<T: Hooks>(
    element: &mut T,
    bounds: Bounds<Pixels>,
    layout: &mut T::Layout,
    prepaint: &mut T::Prepaint,
    window: &mut Window,
    cx: &mut App,
) {
    reblessive::Stack::new()
        .enter(|stack| element.paint_async(bounds, layout, prepaint, window, cx, stack))
        .finish();
}

/// A wrapper around exactly one child: every phase forwards to the child unless overridden.
pub(crate) trait Wrap: 'static {
    fn paint_boundary(&self) -> bool {
        true
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn child(&mut self) -> &mut AnyElement;

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> LayoutId {
        self.child().request_layout_async(window, cx, stack).await
    }

    async fn prepaint_async(
        &mut self,
        _bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        self.child().prepaint_async(window, cx, stack).await;
    }

    async fn paint_async(
        &mut self,
        _bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        self.child().paint_async(window, cx, stack).await;
    }
}

impl<T: Wrap> Hooks for T {
    fn paint_boundary(&self) -> bool {
        Wrap::paint_boundary(self)
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        Wrap::paint_metadata(self)
    }

    fn paint_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        _: &'a mut (),
        _: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(Wrap::paint_async(self, bounds, window, cx, stack))
    }

    type Layout = ();
    type Prepaint = ();

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        (
            Wrap::request_layout_async(self, window, cx, stack).await,
            (),
        )
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        Wrap::prepaint_async(self, bounds, window, cx, stack).await;
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        reblessive::Stack::new()
            .enter(|stack| Wrap::paint_async(self, bounds, window, cx, stack))
            .finish();
    }
}

/// The `gpui::Element` of a [`Hooks`] implementor.
pub(crate) struct Hooked<T>(pub(crate) T);

/// Conversion of a [`Hooks`] implementor into a gpui element.
pub(crate) trait IntoHooked: Hooks + Sized {
    /// The element itself, for `ParentElement::child` and friends.
    fn into_hooked(self) -> Hooked<Self> {
        Hooked(self)
    }

    /// The type-erased element.
    fn hooked(self) -> AnyElement {
        Hooked(self).into_any_element()
    }
}

impl<T: Hooks> IntoHooked for T {}

impl<T: Hooks> IntoElement for Hooked<T> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

mod element;

/// Identity hooks for native elements whose frame state is keyed by `self.id`.
macro_rules! keyed_element_identity {
    () => {
        fn id(&self) -> Option<gpui::ElementId> {
            Some(self.id.clone())
        }
        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }
    };
}
pub(crate) use keyed_element_identity;
