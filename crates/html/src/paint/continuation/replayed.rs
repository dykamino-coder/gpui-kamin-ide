//! Место absolute/fixed корня в записи главной копии разрезанного ребёнка стопки: запись
//! одна на все куски, поэтому слот кладёт в сегмент каждого куска нативную запись
//! продолжения его фрагментаинера, перенесённую в координаты источника записи (воспроизведение
//! куска возвращает её на место), и в prepaint сообщает область попадания для каждого
//! фрагментаинера — как копия этого куска.

use crate::layout::fragment::replay_pieces::ReplayPieces;
use gpui::{Bounds, PaintSegmentId, PaintSegmentSlice, Window, point};
use std::rc::Rc;

/// Prepaint или запись главной копии с кусками `pieces`: слоты внутри видят все куски, а
/// не сегмент одной копии предков.
pub(crate) fn with_pieces<R>(
    pieces: Rc<ReplayPieces>,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    crate::paint::coordinator::segments::outside_source(window, |window| {
        window.with_render_context::<super::CopySegment, R>(None, |window| {
            window.with_render_context::<super::CopyArea, R>(None, |window| {
                window.with_render_context(Some(pieces), f)
            })
        })
    })
}

/// Фрагментаинеры кусков записи, кроме `own`, — для областей попадания слота `root`.
pub(crate) fn columns(window: &Window, own: PaintSegmentId) -> Vec<PaintSegmentId> {
    let Some(all) = window.render_context::<ReplayPieces>() else {
        return Vec::new();
    };
    let mut columns: Vec<_> = all.pieces.iter().map(|p| p.column).collect();
    columns.sort_by_key(|c| c.0);
    columns.dedup();
    columns.retain(|c| *c != own);
    columns
}

/// Нативные записи корня `root` в сегментах кусков записи. Ложь — корня записи не
/// продолжают, слот рисует своего ребёнка.
pub(crate) fn paint(root: u64, window: &mut Window) -> bool {
    let Some(all) = window.render_context::<ReplayPieces>() else {
        return false;
    };
    let Some(records) = all.records.as_ref().filter(|r| r.natives().contains(root)) else {
        return false;
    };
    for piece in all.pieces.iter() {
        let replays = records.get(piece.column, root);
        if replays.is_empty() {
            continue;
        }
        let delta = piece.placement.destination - piece.placement.source;
        let slice = PaintSegmentSlice {
            placement: piece.placement,
            source_clip: Bounds::new(piece.area.origin - delta, piece.area.size),
        };
        let shift = point(-delta.x, -delta.y);
        let _ = window.with_paint_segment_slices(Rc::from([slice]), |window| {
            super::replay(replays, shift, window)
        });
    }
    true
}
