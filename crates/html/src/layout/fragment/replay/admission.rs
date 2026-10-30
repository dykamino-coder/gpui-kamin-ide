//! Собственный допуск узлов в записи главной копии: неделимый монолит, переполнивший
//! кусок своего начала, рисуется целиком в этом фрагментаинере и ни в каком другом
//! (css-break-3 §4.1; Blink кладёт монолит в фрагмент его начала, переполнение — его
//! ink overflow, `BoxFragmentPainter` рисует его там). Остальные узлы наследуют срез
//! своего куска. Элементы flex и сетки — как их копии (`items`).

use super::Source;
use crate::layout::fragment::types::Frag;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, PaintSegment, PaintSegmentSlice, Pixels, point, px};
mod items;
use std::collections::HashMap;
use std::rc::Rc;

/// Допуск узлов ребёнка и расширения масок кусков, где монолиты переполняют срез.
#[derive(Default)]
pub(super) struct Nodes {
    pub(crate) slices: HashMap<u64, Rc<[PaintSegmentSlice]>>,
    /// Размещения сверх размещений кусков: элементы, сдвинутые окном копии.
    pub(super) routes: Vec<PaintSegment>,
    /// Маска воспроизведения куска (по номеру копии) поверх его среза, в координатах окна.
    pub(super) masks: HashMap<usize, Bounds<Pixels>>,
}

/// Монолиты ребёнка `src`, переполнившие кусок своего начала: срез — срез куска,
/// продолженный вниз на всю протяжённость монолита и вылет `spill`.
pub(super) fn nodes(
    src: &Source,
    pieces: &[(&Frag, PaintSegment, Bounds<Pixels>)],
    spill: Pixels,
) -> Nodes {
    let mut out = Nodes::default();
    let (Some(r), Some(w)) = (src.regrow, src.watched) else {
        return out;
    };
    let Some(blocks) = w.blocks() else {
        return out;
    };
    for (id, top, bottom) in
        (crate::layout::fragment::push::monolith_flow::monoliths(blocks, &r.laid))
            .into_iter()
            .filter(|_| src.monoliths)
    {
        let Some(&(f, placement, clip)) = pieces
            .iter()
            .find(|(f, _, _)| f.from - COORD_EPS <= top && top < f.from + f.h - COORD_EPS)
        else {
            continue;
        };
        let end = f.from + f.h;
        if bottom <= end + COORD_EPS {
            continue;
        }
        let source_clip = Bounds::from_corners(
            clip.origin,
            point(clip.right(), clip.bottom() + px(bottom - end) + spill),
        );
        let delta = placement.destination - placement.source;
        let placed = Bounds::new(source_clip.origin + delta, source_clip.size);
        out.masks
            .entry(f.copy)
            .and_modify(|m| *m = m.union(&placed))
            .or_insert(placed);
        out.slices.insert(
            id,
            Rc::from([PaintSegmentSlice {
                placement,
                source_clip,
            }]),
        );
    }
    items::admission(pieces, (w, &r.laid), spill, &mut out);
    out
}
