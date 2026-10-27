//! Контракт immutable record: однократность фаз и явные scopes размещения.

mod clip;
mod consumer;
mod host;
mod lifetime;
mod placement;
mod primitives;
mod scopes;
mod segments;
mod affine_presentation;
mod vector_transform;

fn draw<E: gpui::Element>(make: impl FnOnce() -> E) {
    super::draw_at_scale(Some(1.), make);
}
use gpui::{Bounds, PaintPlacement, Pixels, Window, fill, point, px, rgb, size};

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn placement(x: f32, y: f32, clip: Bounds<Pixels>) -> PaintPlacement {
    PaintPlacement {
        source: point(px(0.), px(0.)),
        destination: point(px(x), px(y)),
        clip,
    }
}

fn quad(window: &mut Window, bounds: Bounds<Pixels>) {
    window.paint_quad(fill(bounds, rgb(0x123456)));
}
