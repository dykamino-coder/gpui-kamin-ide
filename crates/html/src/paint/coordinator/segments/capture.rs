//! Собственная запись внепоточного корня: CB clips в исходных координатах до размещения,
//! голые сегменты для legacy-копий, остальные — обратно в запись источника.

use super::*;

/// Paint a continued root once into its own capture, without its ancestors'
/// effects; re-admit placements that no legacy copy replays.
#[allow(clippy::too_many_arguments)]
pub(super) fn own_capture(
    source: &Rc<Source>,
    id: u64,
    cb: Option<u64>,
    nested: Rc<Nested>,
    key: u64,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) {
    // The containing block's overflow clips inside the fragmented flow apply in
    // source coordinates, before placement (css-overflow-3 §3, css-break-3 §2);
    // clips above the source root stay with the destination.
    let base = source.cb_clips(0, 0);
    let local = cb.and_then(|cb| source.cb_clips(key, cb)).zip(base.clone());
    let captured = window.record_paint_segments(nested.extent, nested.routes.clone(), cx, {
        let source = source.clone();
        let slices = nested
            .families
            .get(&id)
            .cloned()
            .unwrap_or_else(|| Rc::from([]));
        move |window, cx| {
            window.with_render_context(Some(Rc::new(Capture(id))), |window| {
                let own = |window: &mut Window| {
                    window.with_paint_segment_slices(slices, |window| {
                        window.with_render_context(Some(Rc::new(CurrentNode(id))), |window| {
                            paint(window, cx)
                        })
                    })
                };
                let result = match &local {
                    Some((clips, base)) => window
                        .with_paint_source_clips(clips, base, own)
                        .and_then(|result| result),
                    None => own(window),
                };
                if let Err(error) = result {
                    source.fail(error);
                }
            });
        }
    });
    let records = match captured {
        Ok((_, records)) => records,
        Err(error) => return source.fail(error),
    };
    for (segment, record) in records.into_records() {
        if nested.bare.contains(&segment) {
            source.bare.insert(segment, id, record);
            continue;
        }
        let outer: Rc<[_]> = source
            .slices
            .nodes
            .get(&id)
            .into_iter()
            .flat_map(|slices| slices.iter())
            .filter(|slice| slice.placement.id == segment)
            .copied()
            .collect();
        if outer.is_empty() {
            continue;
        }
        let clips = base.clone();
        let replay = move |window: &mut Window| {
            let origin = point(px(0.), px(0.));
            let placement = PaintPlacement {
                source: origin,
                destination: origin,
                clip: window.content_mask().bounds,
            };
            window
                .paint_plan(&PaintPlan::record(record), placement)
                .map(|_| ())
        };
        let result = window.with_paint_segment_slices(outer, |window| match &clips {
            Some(clips) => window
                .with_paint_segment_clips(clips, replay)
                .and_then(|result| result),
            None => replay(window),
        });
        if let Err(error) = result.and_then(|result| result) {
            source.fail(error);
        }
    }
}
