//! Filtered replaced resources share Img layout, then filter their concrete object.

use crate::paint::background::Source;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Img, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};

pub(super) struct FilteredImage {
    image: Img,
    source: Source,
    style: Computed,
}
impl FilteredImage {
    pub(super) fn new(image: Img, source: Source, style: Computed) -> Self {
        Self {
            image,
            source,
            style,
        }
    }
    fn object_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let nat = self.source.intrinsic();
        let (nw, nh) = crate::paint::background::default_size(nat, (bw, bh));
        let k = match self.style.object_fit.as_deref() {
            Some("cover") => (bw / nw).max(bh / nh),
            Some("contain") => (bw / nw).min(bh / nh),
            Some("scale-down") => (bw / nw).min(bh / nh).min(1.0),
            Some("none") => 1.0,
            _ => return bounds,
        };
        let (w, h) = (nw * k, nh * k);
        let position = |l: Option<Len>, free: f32| match l {
            Some(Len::Px(p)) => p,
            Some(Len::Pct(p)) => p * free,
            _ => free * 0.5,
        };
        let pos = self.style.object_position.unwrap_or_default();
        Bounds {
            origin: gpui::point(
                bounds.origin.x + px(position(pos.x, bw - w)),
                bounds.origin.y + px(position(pos.y, bh - h)),
            ),
            size: gpui::size(px(w), px(h)),
        }
    }
}
impl IntoElement for FilteredImage {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FilteredImage {
    type RequestLayoutState = <Img as Element>::RequestLayoutState;
    type PrepaintState = <Img as Element>::PrepaintState;
    fn id(&self) -> Option<ElementId> {
        self.image.id()
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        self.image.source_location()
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.image.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.image
            .prepaint(id, inspector, bounds, state, window, cx)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let object = self.object_bounds(bounds);
        let Some(image) = self.source.sampled_raster(
            (object.size.width.into(), object.size.height.into()),
            self.style.image_pixelated == Some(true),
        ) else {
            return;
        };
        let [tl, tr, br, bl] = super::content_radii::content_radii(&self.style);
        let corners = gpui::Corners {
            top_left: px(tl),
            top_right: px(tr),
            bottom_right: px(br),
            bottom_left: px(bl),
        };
        let visible = self.style.overflow_x == Some(crate::style::computed::Overflow::Visible)
            && self.style.overflow_y == Some(crate::style::computed::Overflow::Visible);
        let mask = (!visible).then_some(gpui::ContentMask { bounds });
        window.with_content_mask(mask, |window| {
            let _ = window.paint_image_with_sampling(
                object,
                corners,
                image,
                0,
                false,
                gpui::ImageSampling::LinearSnapped,
            );
        });
    }
}
