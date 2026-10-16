//! Measured root padding box for canvas background positioning (CSS Backgrounds 3).

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub(crate) type Measurement = Rc<Cell<Option<Bounds<Pixels>>>>;
type WeakMeasurement = Weak<Cell<Option<Bounds<Pixels>>>>;

thread_local! {
    static PENDING: RefCell<Vec<(u64, WeakMeasurement)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn capture(key: u64) -> Measurement {
    let area = Rc::new(Cell::new(None));
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        pending.retain(|(_, area)| area.strong_count() > 0);
        pending.push((key, Rc::downgrade(&area)));
    });
    area
}

fn take(key: u64) -> Option<Measurement> {
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        let index = pending.iter().rposition(|(salt, _)| *salt == key)?;
        pending.remove(index).1.upgrade()
    })
}

/// The child remains the layout node: measuring cannot change margin collapse or sizing.
pub(crate) fn record(child: AnyElement, key: u64, style: &Computed, base_size: f32) -> AnyElement {
    let Some(area) = take(key) else { return child };
    let em = match style.font_size {
        Some(Len::Px(v)) => v,
        _ => base_size,
    };
    let family = style.font_family.clone().unwrap_or_default();
    let side = |v| crate::text::metrics::spacing_px(v, &family, em);
    let b = style.borders();
    RootBox {
        child,
        area,
        border: [side(b.top), side(b.right), side(b.bottom), side(b.left)],
    }
    .into_any_element()
}

struct RootBox {
    child: AnyElement,
    area: Measurement,
    border: [f32; 4],
}

impl Element for RootBox {
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
        let layout = self.child.request_layout(window, cx);
        (layout, layout)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let exact = Bounds {
            origin: window.layout_origin_unrounded(*layout),
            size: window.layout_size_unrounded(*layout),
        };
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        let mut area = if near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom())
        {
            exact
        } else {
            bounds
        };
        let [t, r, b, l] = self.border.map(px);
        area.origin.x += l;
        area.origin.y += t;
        area.size.width = (area.size.width - l - r).max(px(0.0));
        area.size.height = (area.size.height - t - b).max(px(0.0));
        self.area.set(Some(area));
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for RootBox {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
