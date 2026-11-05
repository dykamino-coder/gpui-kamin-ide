//! Допуск узла и выбор сегментов сохранённого источника.

use super::*;
mod traversal;
pub(crate) use traversal::node_async;

#[cfg(test)]
pub(crate) fn node(
    id: u64,
    cb: Option<u64>,
    placed: gpui::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) {
    let Some(source) = window.render_context::<Source>() else {
        paint(window, cx);
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
        Some(by) if by != gpui::Point::default() => window
            .with_render_context(Some(Rc::new(shift::Shift(by))), |window| {
                admitted(source, id, cb, window, cx, paint)
            }),
        _ => admitted(source, id, cb, window, cx, paint),
    }
}

#[cfg(test)]
fn admitted(
    source: Rc<Source>,
    id: u64,
    cb: Option<u64>,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) {
    let key = capture_key(window);
    let by = shift::current(window);
    if source.painting
        && key == 0
        && let Some(nested) = source.slices.nested.get(&id).cloned()
    {
        let nested = shift::nested(nested, by);
        own_capture(&source, id, cb, nested, key, window, cx, paint);
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
    let paint = |window: &mut Window| {
        window.with_render_context(Some(Rc::new(CurrentNode(id))), |window| paint(window, cx));
    };
    let clips = cb.map(|cb| source.cb_clips(key, cb));
    let result = window.with_paint_segment_slices(slices, |window| match clips {
        Some(Some(clips)) => window.with_paint_segment_clips(&clips, paint),
        // Inside an own capture the CB may lie outside: its clip belongs to
        // the enclosing record or the legacy copy that replays this one.
        Some(None) if key != 0 => {
            paint(window);
            Ok(())
        }
        Some(None) => Err(PaintRecordError::InvalidGeometry),
        None => {
            paint(window);
            Ok(())
        }
    });
    if let Err(error) = result.and_then(|result| result) {
        source.fail(error);
    }
}
