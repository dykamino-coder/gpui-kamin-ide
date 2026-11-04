//! Запись фрагментов удерживает источник до завершения продолжения потомков.

use super::*;

pub(crate) async fn record_async(
    extent: Bounds<Pixels>,
    segments: Rc<[PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
    paint: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack),
) -> Result<(PaintSegmentRecords, BareRecords), PaintRecordError> {
    let source = Source::new(slices, true);
    let (_, records) = window.record_paint_segments_async(
        extent, segments, cx, stack, async |window, cx, stack| {
            match window.paint_segment_clips() {
                Ok(clips) => source.remember((0, 0), clips),
                Err(error) => source.fail(error),
            }
            let unadmitted = window.with_paint_segment_slices_async(Rc::from([]), async |window| {
                window.with_render_context_async(Some(source.clone()), async |window| {
                    window.with_render_context_async::<crate::layout::fragment::line_admission::Lines, _>(None, async |window| {
                        paint(window, cx, stack).await;
                    }).await;
                }).await;
            }).await;
            if let Err(error) = unadmitted {
                source.fail(error);
            }
        },
    ).await?;
    if let Some(error) = source.failure.take() {
        return Err(error);
    }
    Ok((records, source.bare.clone()))
}
