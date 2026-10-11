//! Пробы размеров детей полос: высота при ширине, внутренние ширины (min/max-content).

use super::kid::{Kid, Kind};
use super::tap::Tap;
use super::{Build, Shapes, frame};
use crate::layout::float::band_flow::VERT;
use gpui::{App, AvailableSpace, IntoElement, LayoutId, Window, px, size};
use std::cell::Cell;
use std::rc::Rc;

/// Пробная раскладка ребёнка при доступной ширине `avail`: размер border-box.
pub(super) fn probe(
    kid: &Kid,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    window: &mut Window,
    cx: &mut App,
) -> (f32, f32) {
    probe_of(kid.kind, &kid.build, cb, avail, shapes, window, cx)
}

/// `probe` для любого построителя: каркас по виду `kind`.
pub(super) fn probe_of(
    kind: Kind,
    build: &Build,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    window: &mut Window,
    cx: &mut App,
) -> (f32, f32) {
    let tap = Rc::new(Cell::new(None));
    let mut el = frame(kind, avail, build(cb, avail, shapes, None), tap.clone());
    let vert = VERT.with(Cell::get).is_some();
    let room = AvailableSpace::Definite(px(avail.max(0.0)));
    let space = if vert {
        size(AvailableSpace::MaxContent, room)
    } else {
        size(room, AvailableSpace::MaxContent)
    };
    el.layout_as_root(space, window, cx);
    // (строчный, блочный) размер: в вертикальном письме — (высота, ширина).
    let (w, h) = unrounded(&tap, window);
    if vert { (h, w) } else { (w, h) }
}

/// Размер ребёнка по его `LayoutId` — БЕЗ округления к физической точке:
/// полосы складывают ширины, и округление каждой копится (`units-005`:
/// десять флоатов по `0.87em` в `8.7em`).
pub(super) fn unrounded(tap: &Rc<Cell<Option<LayoutId>>>, window: &mut Window) -> (f32, f32) {
    match tap.get() {
        Some(id) => {
            let s = window.layout_size_unrounded(id);
            (f32::from(s.width), f32::from(s.height))
        }
        None => (0.0, 0.0),
    }
}

/// Внутренние ширины ребёнка (min-content, max-content) border-box.
pub(super) fn intrinsic(kid: &Kid, window: &mut Window, cx: &mut App) -> (f32, f32) {
    intrinsic_of(&kid.build, window, cx)
}

/// Внутренние ширины того, что строит `build` (ребёнок или голова прогона).
pub(super) fn intrinsic_of(build: &Build, window: &mut Window, cx: &mut App) -> (f32, f32) {
    let tap = Rc::new(Cell::new(None));
    let mut el = Tap {
        inner: build(0.0, 0.0, None, None),
        id: tap.clone(),
    }
    .into_any_element();
    let vert = VERT.with(Cell::get).is_some();
    let mut w = |a: AvailableSpace, window: &mut Window| {
        if vert {
            // Повёрнутый текст свою длину высотой не заявляет (растягивается
            // окном) — берём её у самого текста через сборщик
            // `VT_INLINE_MAX`; коробка без текста отвечает раскладкой.
            let prev = crate::text::vertical::VT_INLINE_MAX.with(|c| c.replace(Some(0.0)));
            el.layout_as_root(size(AvailableSpace::MaxContent, a), window, cx);
            let text = crate::text::vertical::VT_INLINE_MAX
                .with(|c| c.replace(prev))
                .unwrap_or(0.0);
            let laid = unrounded(&tap, window).1;
            if text > 0.0 { text.min(laid) } else { laid }
        } else {
            el.layout_as_root(size(a, AvailableSpace::MaxContent), window, cx);
            unrounded(&tap, window).0
        }
    };
    let mn = w(AvailableSpace::MinContent, window);
    let mx = w(AvailableSpace::MaxContent, window);
    (mn, mx.max(mn))
}
