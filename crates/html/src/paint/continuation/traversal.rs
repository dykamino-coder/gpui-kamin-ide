//! Области копии живут до завершения дочернего обхода на общем стеке.

use super::*;

pub(crate) async fn with_copy_hits_async<R>(
    segment: PaintSegmentId,
    scopes: &SlotScopes,
    window: &mut Window,
    prepaint: impl AsyncFnOnce(&mut Window) -> R,
) -> R {
    let context = CopyHits {
        segment,
        scopes: scopes.clone(),
    };
    window
        .with_render_context_async(Some(Rc::new(context)), prepaint)
        .await
}

pub(crate) async fn with_copy_segment_async(
    segment: PaintSegmentId,
    records: Option<&BareRecords>,
    window: &mut Window,
    paint: impl AsyncFnOnce(&mut Window),
) {
    match records {
        Some(records) => {
            window
                .with_render_context_async(
                    Some(Rc::new(CopySegment {
                        segment,
                        records: records.clone(),
                    })),
                    paint,
                )
                .await
        }
        None => paint(window).await,
    }
}

pub(crate) async fn with_copy_area_async(
    area: gpui::ContentMask<Pixels>,
    window: &mut Window,
    paint: impl AsyncFnOnce(&mut Window),
) {
    window
        .with_render_context_async(Some(Rc::new(CopyArea(area))), paint)
        .await
}
