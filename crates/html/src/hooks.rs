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
pub(crate) use paint::{paint_collect, paint_inline};
mod painter;
pub(crate) use paint::BareRecords;
pub(crate) use paint::{SlotScopes, presented_hit_scopes, with_copy_hits, with_copy_segment};
pub(crate) use paint::{
    SourceSlices, paint_source_node, paint_source_record, prepaint_source_record,
};
pub(crate) use paint::{paint_atomic, paint_last, paint_reopen, paint_root};
pub(crate) use painter::{PaintOnly, Painter};

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

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout);

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint;

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    );
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

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        self.child().request_layout(window, cx)
    }

    fn prepaint(&mut self, _bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.child().prepaint(window, cx);
    }

    fn paint(&mut self, _bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.child().paint(window, cx);
    }
}

impl<T: Wrap> Hooks for T {
    fn paint_boundary(&self) -> bool {
        Wrap::paint_boundary(self)
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        Wrap::paint_metadata(self)
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (Wrap::request_layout(self, window, cx), ())
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        Wrap::prepaint(self, bounds, window, cx);
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        Wrap::paint(self, bounds, window, cx);
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

impl<T: Hooks> Element for Hooked<T> {
    fn paint_boundary(&self) -> bool {
        self.0.paint_boundary()
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        self.0.paint_metadata()
    }

    type RequestLayoutState = T::Layout;
    type PrepaintState = T::Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, T::Layout) {
        Hooks::request_layout(&mut self.0, window, cx)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut T::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> T::Prepaint {
        Hooks::prepaint(&mut self.0, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut T::Layout,
        prepaint: &mut T::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        Hooks::paint(&mut self.0, bounds, layout, prepaint, window, cx);
    }
}

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
