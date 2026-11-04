//! Допуск сохраняется в продолжении краски источника.

use super::*;

pub(crate) async fn node_async(
    id: u64,
    cb: Option<u64>,
    placed: gpui::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
    paint: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack),
) {
    let Some(source) = window.render_context::<Source>() else {
        paint(window, cx, stack).await;
        return;
    };
    // Its own instance outside the source paints it (`SourceSlices::elsewhere`).
    if source.slices.elsewhere.contains(&id) {
        return;
    }
    // A continued root placed elsewhere than its admission's layout (a late
    // static position) carries its family's admission along.
    match source
        .slices
        .origins
        .get(&id)
        .map(|origin| placed - *origin)
    {
        Some(by) if by != gpui::Point::default() => {
            window
                .with_render_context_async(Some(Rc::new(shift::Shift(by))), async |window| {
                    admitted_async(source, id, cb, window, cx, stack, paint).await
                })
                .await
        }
        _ => admitted_async(source, id, cb, window, cx, stack, paint).await,
    }
}

fn admitted_async<'a>(
    source: Rc<Source>,
    id: u64,
    cb: Option<u64>,
    window: &'a mut Window,
    cx: &'a mut App,
    stack: &'a mut gpui::PaintStack,
    paint: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack) + 'a,
) -> gpui::PaintFuture<'a> {
    Box::pin(async move {
        let key = capture_key(window);
        let by = shift::current(window);
        if source.painting
            && key == 0
            && let Some(nested) = source.slices.nested.get(&id).cloned()
        {
            let nested = shift::nested(nested, by);
            super::super::capture::own_capture_async(
                &source, id, cb, nested, key, window, cx, stack, paint,
            )
            .await;
            return;
        }
        let slices = match source.own(id, key) {
            // A capture's own admission was shifted with its root already.
            Some(slices) => slices,
            None if key != 0 => Rc::from([]),
            None => shift::slices(
                source
                    .slices
                    .nodes
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| Rc::from([])),
                by,
            ),
        };
        let paint = async |window: &mut Window| {
            window
                .with_render_context_async(Some(Rc::new(CurrentNode(id))), async |window| {
                    paint(window, cx, stack).await
                })
                .await;
        };
        let clips = cb.map(|cb| source.cb_clips(key, cb));
        let result = window
            .with_paint_segment_slices_async(slices, async |window| match clips {
                Some(Some(clips)) => window.with_paint_segment_clips_async(&clips, paint).await,
                // Inside an own capture the CB may lie outside: its clip belongs to
                // the enclosing record or the legacy copy that replays this one.
                Some(None) if key != 0 => {
                    paint(window).await;
                    Ok(())
                }
                Some(None) => Err(PaintRecordError::InvalidGeometry),
                None => {
                    paint(window).await;
                    Ok(())
                }
            })
            .await;
        if let Err(error) = result.and_then(|result| result) {
            source.fail(error);
        }
    })
}
