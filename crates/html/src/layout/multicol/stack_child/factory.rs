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
    frag_geom: Vec<(f32, f32)>,
    dec: Option<(f32, f32)>,
    merged: &BoxStyle,
    copy: &Element,
    h: f32,
    over: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: BoxStyle,
    whole: bool,
    watched: Rc<Watched>,
) -> Builder {
    let opts = opts.clone();
    let merged = merged.clone();
    let copy = copy.clone();
    std::rc::Rc::new(move |first: bool, part: usize, band: bool| {
        let mut opts = opts.in_stack_copy();
        // Копии фрагментов flex тоже наблюдаются: элемент рисует только
        // копия его фрагментов (`Watched::set_flex_owners`).
        opts.frag.watch = if first && !band {
            Some(watched.clone())
        } else if !band && watched.blocks().is_some_and(|b| b.has_flex()) {
            Some(watched.for_part(part))
        } else {
            None
        };
        build::build_copy(
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
        )
    })
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
