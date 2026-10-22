//! Пробы размеров детей полос: высота при ширине, внутренние ширины (min/max-content).

use super::kid::{Kid, Kind};
use super::tap::Tap;
use super::{Build, Probe, Shapes, frame};
use crate::hooks::IntoHooked;
use gpui::{AvailableSpace, LayoutId, Window, px, size};
use std::cell::Cell;
use std::rc::Rc;

/// Пробная раскладка ребёнка при доступной ширине `avail`: размер border-box.
pub(super) fn probe(
    kid: &Kid,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    p: &mut Probe,
) -> (f32, f32) {
    probe_of(kid.kind, &kid.build, cb, avail, shapes, p)
}

/// `probe` для любого построителя: каркас по виду `kind`.
pub(super) fn probe_of(
    kind: Kind,
    build: &Build,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    p: &mut Probe,
) -> (f32, f32) {
    let _flow_scope = p.fragment.as_ref().map(|f| f.probes.enter());
    let tap = Rc::new(Cell::new(None));
    let vert = p.vert;
    let mut el = frame(
        kind,
        avail,
        build(cb, avail, shapes, None, None),
        tap.clone(),
        vert,
        &p.paint,
        &p.build,
    );
    let room = AvailableSpace::Definite(px(avail.max(0.0)));
    let space = if vert {
        size(AvailableSpace::MaxContent, room)
    } else {
        size(room, AvailableSpace::MaxContent)
    };
    el.layout_as_root(space, p.window, p.cx);
    // (строчный, блочный) размер: в вертикальном письме — (высота, ширина).
    let (w, h) = unrounded(&tap, p.window);
    p.last_geometry = match (&p.fragment, tap.get()) {
        (Some(f), Some(id)) => f.probes.relative(id, f.node.axis, p.window),
        _ => Default::default(),
    };
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
pub(super) fn intrinsic(kid: &Kid, p: &mut Probe) -> (f32, f32) {
    intrinsic_of(&kid.build, p)
}

/// Внутренние ширины того, что строит `build` (ребёнок или голова прогона).
pub(super) fn intrinsic_of(build: &Build, p: &mut Probe) -> (f32, f32) {
    let _flow_scope = p.fragment.as_ref().map(|f| f.probes.enter());
    let tap = Rc::new(Cell::new(None));
    let measure = p.vert.then(crate::text::vertical::InlineMeasure::default);
    let mut el = Tap {
        inner: build(0.0, 0.0, None, None, measure.clone()),
        id: tap.clone(),
    }
    .hooked();
    let vert = p.vert;
    let mut w = |a: AvailableSpace, p: &mut Probe| {
        if vert {
            // Повёрнутый текст свою длину высотой не заявляет (растягивается
            // окном) — берём её у самого текста через сборщик
            // intrinsic measurement; коробка без текста отвечает раскладкой.
            if let Some(measure) = &measure {
                measure.reset();
            }
            el.layout_as_root(size(AvailableSpace::MaxContent, a), p.window, p.cx);
            let text = measure.as_ref().map_or(0.0, |measure| measure.extent());
            let laid = unrounded(&tap, p.window).1;
            if text > 0.0 { text.min(laid) } else { laid }
        } else {
            el.layout_as_root(size(a, AvailableSpace::MaxContent), p.window, p.cx);
            unrounded(&tap, p.window).0
        }
    };
    let mn = w(AvailableSpace::MinContent, p);
    let mx = w(AvailableSpace::MaxContent, p);
    (mn, mx.max(mn))
}
