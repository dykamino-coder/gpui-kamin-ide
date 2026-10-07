//! Solid rectangular outlines snap their inner and outer edges together.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, ParentElement, Pixels, Styled, Window, div, point, px,
};

pub(super) fn element(
    border: [f32; 4],
    offset: f32,
    width: f32,
    corner: f32,
    color: Hsla,
    style: Option<u8>,
) -> AnyElement {
    let [top, right, bottom, left] = border;
    let mut ring = div()
        .absolute()
        .top(px(-(offset + width + top)))
        .left(px(-(offset + width + left)))
        .right(px(-(offset + width + right)))
        .bottom(px(-(offset + width + bottom)))
        .rounded(px(corner));
    if corner == 0.0 && matches!(style, Some(1 | 2)) {
        ring = ring.child(Ring {
            child: div().absolute().size_full().into_any_element(),
            width,
            color,
        });
    } else {
        ring = ring.border(px(width)).border_color(color);
        // Patterned outlines keep the same primitive as patterned CSS borders.
        match style {
            Some(3) => ring.style().border_style = Some(gpui::BorderStyle::Dotted),
            Some(4) => ring = ring.border_dashed(),
            _ => {}
        }
    }
    ring.into_any_element()
}

struct Ring {
    child: AnyElement,
    width: f32,
    color: Hsla,
}

impl IntoElement for Ring {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Ring {
    type RequestLayoutState = LayoutId;
    type PrepaintState = Bounds<Pixels>;

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
    ) -> (LayoutId, LayoutId) {
        let id = self.child.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Bounds<Pixels> {
        self.child.prepaint(window, cx);
        if window.current_transformation() != gpui::TransformationMatrix::unit() {
            return bounds;
        }
        // Derive both contours from layout coordinates. Deriving the inner
        // contour from a snapped outer origin would round a half pixel twice.
        Bounds {
            origin: window.layout_origin_unrounded(*id),
            size: window.layout_size_unrounded(*id),
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        fallback: Bounds<Pixels>,
        _: &mut LayoutId,
        bounds: &mut Bounds<Pixels>,
        window: &mut Window,
        _: &mut App,
    ) {
        let bounds = if window.current_transformation() == gpui::TransformationMatrix::unit() {
            *bounds
        } else {
            fallback
        };
        paint(bounds, self.width, self.color, window);
    }
}

fn paint(bounds: Bounds<Pixels>, width: f32, color: Hsla, window: &mut Window) {
    if window.current_transformation() != gpui::TransformationMatrix::unit() {
        let mut quad = gpui::outline(bounds, color, gpui::BorderStyle::Solid);
        quad.border_widths = gpui::Edges::all(px(width));
        window.paint_quad(quad);
        return;
    }
    // CSS UI 4 §3 defines the outline outside its offset edge. Blink likewise
    // uses pixel-snapped outline rectangles (outline_painter.cc:967). A ring
    // with an empty interior has no antialiased hole or shared-edge seam.
    let scale = window.scale_factor();
    let edge = |v: Pixels| px((f32::from(v) * scale + 0.5).floor() / scale);
    let (l, t, r, b) = (
        edge(bounds.left()),
        edge(bounds.top()),
        edge(bounds.right()),
        edge(bounds.bottom()),
    );
    let (il, it, ir, ib) = (
        edge(bounds.left() + px(width)),
        edge(bounds.top() + px(width)),
        edge(bounds.right() - px(width)),
        edge(bounds.bottom() - px(width)),
    );
    let mut fill = |l, t, r, b| {
        if r > l && b > t {
            window.paint_quad(gpui::fill(
                Bounds::from_corners(point(l, t), point(r, b)),
                color,
            ));
        }
    };
    if il >= ir || it >= ib {
        fill(l, t, r, b);
    } else {
        // Nonoverlapping strips also preserve translucent outline colors.
        fill(l, t, r, it);
        fill(l, ib, r, b);
        fill(l, it, il, ib);
        fill(ir, it, r, ib);
    }
}
