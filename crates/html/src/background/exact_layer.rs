//! Position a box's background layer from its unrounded layout box.
//!
//! CSS 2.1 section 14.2 and css-backgrounds-3 section 3.6 position a background
//! in the box's positioning area; Blink resolves `background-position` against
//! the unsnapped area (background_image_geometry.cc: "Unsnapped positioning
//! area is used to derive quantities ... such as phase and position") and
//! snaps only the destination. The layer canvas received the device-snapped
//! box, so a centred tile moved by up to half a device pixel against the same
//! tile in a box of equal exact geometry (background-root-013a/b).
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Styled as _, Window, px,
};
use std::cell::Cell;

thread_local! {
    /// The layer being painted: its snapped bounds and the unrounded ones.
    static EXACT: Cell<Option<(Bounds<Pixels>, Bounds<Pixels>)>> = const { Cell::new(None) };
}

/// The positioning box for `bounds`: the unrounded box when `bounds` is the
/// snapped box of the layer being painted, otherwise `bounds` unchanged.
pub(super) fn positioning(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    match EXACT.with(Cell::get) {
        Some((snapped, exact)) if snapped == bounds => exact,
        _ => bounds,
    }
}

type PaintFn = Box<dyn FnMut(Bounds<Pixels>, &mut Window)>;

pub(super) struct ExactLayer {
    child: Option<AnyElement>,
    paint: PaintFn,
    exact: Option<Bounds<Pixels>>,
}

impl ExactLayer {
    pub(super) fn new(paint: impl FnMut(Bounds<Pixels>, &mut Window) + 'static) -> Self {
        Self {
            child: Some(
                gpui::div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .into_any_element(),
            ),
            paint: Box::new(paint),
            exact: None,
        }
    }
}

impl Element for ExactLayer {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let exact = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        // Only an unrounded box within a device pixel of the snapped one is
        // the same box (another frame of reference otherwise).
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        self.exact = (near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom()))
        .then_some(exact);
        self.child.as_mut().unwrap().prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let outer = EXACT.with(|cell| cell.replace(self.exact.map(|exact| (bounds, exact))));
        (self.paint)(bounds, window);
        EXACT.with(|cell| cell.set(outer));
    }
}

impl IntoElement for ExactLayer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
