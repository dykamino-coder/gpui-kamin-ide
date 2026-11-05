//! Пробы размеров детей полос: высота при ширине, внутренние ширины (min/max-content).

use super::kid::{Kid, Kind};
use super::{Build, Probe, Shapes, frame};
use gpui::{AvailableSpace, LayoutId, Window, px, size};
use std::cell::Cell;
use std::rc::Rc;

/// Пробная раскладка ребёнка при доступной ширине `avail`: размер border-box.
pub(super) async fn probe(
    kid: &Kid,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    let old = p.fragment.as_ref().map(|f| f.probes.clone());
    if let Some(watched) = &kid.watched {
        watched.reset();
    }
    if let (Some(fragment), Some(flow)) = (&mut p.fragment, &kid.flow) {
        flow.rewind();
        fragment.probes = flow.clone();
    }
    let size = probe_of(
        kid.kind,
        &kid.build,
        cb,
        avail,
        shapes,
        kid.watched.as_deref(),
        p,
        stack,
    )
    .await;
    if let (Some(fragment), Some(old)) = (&mut p.fragment, old) {
        fragment.probes = old;
    }
    size
}

/// `probe` для любого построителя: каркас по виду `kind`.
pub(super) async fn probe_of(
    kind: Kind,
    build: &Build,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    watched: Option<&crate::layout::fragment::paginate::watch::Watched>,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    // Готовое дерево плана живёт до paint; только числовая проба может
    // освобождать свои элементы сразу после измерения.
    if p.engine.is_some() {
        probe_layout(kind, build, cb, avail, shapes, watched, p, stack).await
    } else {
        p.temporary_layout(stack, async |p, stack| {
            stack
                .run(|stack| probe_layout(kind, build, cb, avail, shapes, watched, p, stack))
                .await
        })
        .await
    }
}

async fn probe_layout(
    kind: Kind,
    build: &Build,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    watched: Option<&crate::layout::fragment::paginate::watch::Watched>,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    crate::work::add!(BandProbeCalls, 1);
    let positioning = p.positioning.clone();
    let _positioning = positioning.as_ref().map(|p| p.isolate());
    let clamp_scope = p.clamp.as_ref().map(|(key, m)| m.enter(*key));
    let _flow_scope = p.fragment.as_ref().map(|f| f.probes.enter());
    let tap = Rc::new(Cell::new(None));
    let vert = p.vert;
    crate::work::add!(FloatChildBuilds, 1);
    let child = {
        crate::work::time!(_build, BandProbeBuildNs);
        build(cb, avail, shapes, None, None)
    };
    let mut el = frame(kind, avail, child, tap.clone(), vert, &p.paint, &p.build);
    let room = AvailableSpace::Definite(px(avail.max(0.0)));
    let space = if vert {
        size(AvailableSpace::MaxContent, room)
    } else {
        size(room, AvailableSpace::MaxContent)
    };
    crate::work::add!(FloatChildLayouts, 1);
    let root_size = {
        crate::work::time!(_layout, BandProbeLayoutNs);
        stack
            .run(|stack| el.layout_as_root_unrounded_async(space, p.window, p.cx, stack))
            .await
    };
    // (строчный, блочный) размер: в вертикальном письме — (высота, ширина).
    let (w, h) = unrounded(&tap, p.window);
    p.last_geometry = match (&p.fragment, tap.get()) {
        (Some(f), Some(id)) => f.probes.relative(id, f.node.axis, p.window),
        _ => Default::default(),
    };
    // LayoutId временной пробы нельзя читать после освобождения `el`.
    // Сохраняем числа и для обычных блочных потомков без собственных строк.
    if let (Some(watched), Some(f), Some(id)) = (watched, &p.fragment, tap.get())
        && let Some(bounds) =
            crate::layout::fragment::geometry::Node::capture(id, p.window).bounds(p.window)
    {
        p.last_geometry.boxes.extend(
            watched
                .laid_axis(bounds.origin, bounds.size, f.node.axis, p.window)
                .boxes,
        );
    }
    p.last_clamp = match (clamp_scope, tap.get()) {
        (Some(scope), Some(id)) => scope.finish(id, p.window),
        _ => Default::default(),
    };
    p.last_prepared = p.engine.as_ref().map_or_else(Default::default, |engine| {
        super::prepared::Prepared::new(
            el,
            engine.clone(),
            p.window.element_id_context(),
            space,
            root_size,
        )
    });
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
