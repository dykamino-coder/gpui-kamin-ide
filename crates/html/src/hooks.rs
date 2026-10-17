//! The engine's one `gpui::Element` adapter: wrappers and probes implement [`Hooks`] (typed
//! lifecycle state) or [`Wrap`] (one child, forwarding defaults) and become elements through
//! [`Hooked`].
//!
//! gpui's `Element` asks every implementor for an id, a source location, two ignored id
//! parameters per phase and an `IntoElement` impl. Engine wrappers never carry an id, so that
//! frame lives here once. Elements that keep gpui element state across frames (keyed by the
//! `GlobalElementId`) — `Transition`, `Selectable`, `Resizable`, `ScrollArea`, `Paragraph` —
//! and `FilteredImage` (delegates to gpui `Img` with its ids) implement `gpui::Element`
//! directly.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

/// The three gpui phases with typed state and without element ids.
pub(crate) trait Hooks: 'static {
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
