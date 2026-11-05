//! Неизменяемый рецепт копии фрагмента; построение запускает только нужный фрагмент.

use super::*;
use crate::layout::fragment::paginate::watch::Watched;
use std::rc::Rc;

pub(super) type Builder = Rc<dyn Fn(bool, usize, bool) -> gpui::AnyElement>;

#[allow(clippy::too_many_arguments)]
pub(super) fn new(
    opts: &RenderOpts,
    col_vert: bool,
    col_rl: bool,
    kid_w: Option<f32>,
    frag_geom: crate::layout::fragment::types::CloneGeom,
    merged: &BoxStyle,
    copy: &Element,
    h: f32,
    over: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: BoxStyle,
    whole: bool,
    watched: Rc<Watched>,
    report: crate::layout::fragment::token::space::sequence::NestReport,
) -> Builder {
    let opts = opts.clone();
    // Under a native source of this multicol every continued absolute/fixed
    // root replays its own continuation record (paint::continuation).
    let slots = opts
        .cx
        .position
        .geometry
        .as_ref()
        .filter(|g| g.native_along(col_vert).is_some() && g.host == merged.self_node)
        .map(|g| g.continued_roots());
    let positioning = opts.cx.position.state.retain();
    let merged = merged.clone();
    let copy = copy.clone();
    std::rc::Rc::new(move |first: bool, part: usize, band: bool| {
        let mut opts = opts.in_stack_copy();
        // A repeated header/footer band clones its monolithic content, absolute
        // boxes included: they repeat with the band instead of continuing.
        opts.cx
            .position
            .continue_in_slots(slots.clone().filter(|_| !band));
        // Копия строится после закрытия слоёв предков. Измерительные деревья
        // изолированы; только prepaint публикует OOF в исходный containing block.
        let (state, escaped) = positioning.isolate();
        opts.cx.position.state = state;
        // Копии фрагментов flex тоже наблюдаются: элемент рисует только
        // копия его фрагментов (`Watched::set_flex_owners`).
        opts.frag.watch = if first && !band {
            Some(watched.clone())
        } else if !band && watched.blocks().is_some_and(|b| b.has_flex()) {
            Some(watched.for_part(part))
        } else {
            None
        };
        let (dec, frag_geom, over) = match &*frag_geom.borrow() {
            Some((dec, geom, end)) => (Some(*dec), geom.clone(), *end),
            None => (None, Vec::new(), over),
        };
        escaped.wrap(build::build_copy(
            &opts,
            col_vert,
            col_rl,
            kid_w,
            &frag_geom,
            dec,
            &merged,
            &copy,
            h,
            over,
            nest_row,
            nest_phase_k,
            &inner,
            whole,
            first,
            part,
            band,
            &report,
        ))
    })
}

#[cfg(test)]
mod tests;

/// Отложенная копия part (0 — главная) того же рецепта, что у ragments.
pub(super) fn copy(
    build: Builder,
    copy: &Element,
    vertical: bool,
    part: usize,
) -> gpui::AnyElement {
    let copy = copy.clone();
    crate::layout::fragment::types::FragmentHandle::new(move || {
        crate::layout::fragment::line_shape::side_margin_wrap(
            build(part == 0, part, false),
            &copy,
            vertical,
        )
    })
    .element()
}

/// Построители разделяют DOM по COW, но у каждой копии собственные элементы и layout-id.
pub(super) fn fragments(
    build: Builder,
    copy: &Element,
    vertical: bool,
    count: usize,
) -> (
    gpui::AnyElement,
    Vec<gpui::AnyElement>,
    Vec<crate::layout::fragment::types::FragmentHandle>,
) {
    use crate::layout::fragment::types::FragmentHandle;
    let mut handles = Vec::with_capacity(count);
    let mut elements = Vec::with_capacity(count);
    for part in 0..count {
        let build = build.clone();
        let copy = copy.clone();
        let handle = FragmentHandle::new(move || {
            crate::layout::fragment::line_shape::side_margin_wrap(
                build(part == 0, part, false),
                &copy,
                vertical,
            )
        });
        elements.push(handle.element());
        handles.push(handle);
    }
    let el = elements.remove(0);
    (el, elements, handles)
}
