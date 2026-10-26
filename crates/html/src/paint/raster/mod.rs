//! CSS-выбор raster frame для заливки бокса и подложки его текста.

mod fill;
pub(crate) use fill::bounds as fill_bounds;

use crate::text::raster::{Background, ReplicatedBackground};
use gpui::{ChildPaintContext, Fill, TransformationMatrix, Window, px};
use std::rc::Rc;

pub(super) fn with_background<R>(
    context: &ChildPaintContext<'_>,
    window: &mut Window,
    paint: impl FnOnce(&mut Window) -> R,
) -> R {
    let style = context.style;
    let bounds = context.bounds;
    let opaque = style
        .background
        .as_ref()
        .and_then(Fill::color)
        .is_some_and(|fill| fill.as_solid().is_some_and(|color| color.a == 1.0));
    let replicated = context
        .metadata
        .is_some_and(|tag| tag.is::<ReplicatedBackground>());
    let eligible = style.snap_to_device_pixels
        && !replicated
        && opaque
        && window.element_opacity() == 1.0
        && style.corner_radii == Default::default()
        && bounds.size.width > px(0.0)
        && bounds.size.height > px(0.0);
    if !eligible {
        return paint(window);
    }
    let exact = context.exact_bounds.map_or(bounds, |(_, exact)| exact);
    let fill_bounds = if window.current_transformation() == TransformationMatrix::unit() {
        fill::bounds_with_exact(style, bounds, context.exact_bounds, window)
    } else {
        bounds
    };
    let mut frame = window.raster_frame().clone();
    let background = Rc::new(Background {
        bounds: exact,
        offset: exact.origin - fill_bounds.origin,
        transform: window.current_transformation(),
        parent: frame.context(),
    });
    frame.set_context(Some(background));
    window.with_raster_frame(frame, paint)
}
