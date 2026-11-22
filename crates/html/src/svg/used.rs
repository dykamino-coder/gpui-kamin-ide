//! Resolve the CSS viewport before creating the SVG raster.

use crate::dom::Element;
use crate::hooks::{Hooks, IntoHooked};
use crate::layout::cx::{FrameCx, FrameDoc};
use crate::style::values::value::Len;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, Pixels, Style, Window, point, px, relative,
    size,
};

pub(super) fn element(e: &Element, frame: FrameCx<'_>) -> AnyElement {
    UsedSvg {
        element: e.clone(),
        frame: frame.keep(),
    }
    .hooked()
}

pub(super) fn needs_used_size(e: &Element) -> bool {
    let b = &e.style.used().reset_box_layout;
    matches!(b.width, Some(Len::Pct(_)))
        || matches!(b.height, Some(Len::Pct(_)))
        || matches!(
            (b.width, b.height),
            (Some(Len::Px(_)), Some(Len::Auto)) | (Some(Len::Auto), Some(Len::Px(_)))
        )
}

struct UsedSvg {
    element: Element,
    frame: FrameDoc,
}

fn layout_style(e: &Element) -> (Style, gpui::Size<Pixels>) {
    let (w, h) = super::size_of(e);
    let natural = size(px(w), px(h));
    let side = |v: Option<Len>, fallback: Option<f32>| match v {
        Some(Len::Pct(k)) => relative(k).into(),
        Some(Len::Px(v)) => px(v).into(),
        Some(Len::Auto) => gpui::Length::Auto,
        _ => fallback.map_or(gpui::Length::Auto, |v| px(v).into()),
    };
    let facts = crate::dom::resource_facts::SvgFacts::of(e);
    let natural_ratio = facts
        .width
        .zip(facts.height)
        .map(|(w, h)| w / h)
        .filter(|r| r.is_finite() && *r > 0.0)
        .or_else(|| facts.view_box.map(|(w, h)| w / h));
    let c = e.style.used();
    let ratio = c
        .reset_shapes
        .aspect_ratio
        .or(natural_ratio)
        .or(c.reset_shapes.aspect_ratio_auto)
        .filter(|r| r.is_finite() && *r > 0.0);
    let mut style = Style {
        size: size(
            side(c.reset_box_layout.width, facts.width.map(|_| w)),
            side(c.reset_box_layout.height, facts.height.map(|_| h)),
        ),
        aspect_ratio: ratio,
        flex_shrink: 0.0,
        ..Style::default()
    };
    // Percentages belong to the CSS box rather than the natural raster size.
    for (target, source) in [
        (
            &mut style.min_size,
            (c.reset_box_layout.min_width, c.reset_box_layout.min_height),
        ),
        (
            &mut style.max_size,
            (c.reset_box_layout.max_width, c.reset_box_layout.max_height),
        ),
    ] {
        *target = size(side(source.0, None), side(source.1, None));
    }
    (style, natural)
}

impl Hooks for UsedSvg {
    type Layout = LayoutId;
    type Prepaint = Option<AnyElement>;

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, LayoutId) {
        let (style, natural) = layout_style(&self.element);
        let ratio = style.aspect_ratio;
        let id = window.request_measured_layout(style, move |known, _, _, _| {
            match (known.width, known.height, ratio) {
                (Some(w), Some(h), _) => size(w, h),
                (Some(w), None, Some(r)) => size(w, px(f32::from(w) / r)),
                (None, Some(h), Some(r)) => size(px(f32::from(h) * r), h),
                (Some(w), None, None) => size(w, natural.height),
                (None, Some(h), None) => size(natural.width, h),
                _ => natural,
            }
        });
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        // Pass the unrounded used size to viewBox serialization and rasterization.
        let dimensions = window.layout_size_unrounded(*id);
        let origin = window.layout_origin_unrounded(*id);
        let mut image = super::element_sized(
            &self.element,
            self.frame.frame(),
            (dimensions.width.into(), dimensions.height.into()),
        )?;
        let root = image.request_layout(window, cx);
        window.compute_layout(
            root,
            size(
                AvailableSpace::Definite(dimensions.width),
                AvailableSpace::Definite(dimensions.height),
            ),
            cx,
        );
        window.set_layout_root_origin(root, origin);
        image.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        Some(image)
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        image: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(image) = image {
            image.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests;
