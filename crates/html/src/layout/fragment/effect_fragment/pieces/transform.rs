//! Плоское преобразование разрезанного ребёнка в записи главной копии: вложенная запись по
//! кускам, возвращаемая в каждый кусок своей матрицей (css-break-3 §5.5).

use super::{Piece, Pieces};
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintSegment, PaintSegmentSlice, Pixels,
    TransformationMatrix, Window, point, px, size,
};
use std::rc::Rc;

/// Плоское преобразование коробки `bounds` по кускам. Поддерево пишется один раз
/// вложенной записью по сегментам кусков (уже на месте каждого куска); каждый сегмент
/// возвращается в запись главной копии под своей матрицей `matrix(reference)` вокруг
/// опорной коробки своего фрагмента (`matrices` — по кускам, в точках устройства, в
/// координатах источника; `list` — `Pieces::current`). Как у копии: при осевой матрице
/// без обрезки предка срез куска стоит до преобразования, иначе — после него, маской куска.
pub(crate) fn paint_transformed(
    pieces: &Pieces,
    (list, matrices): (&[Piece], &[TransformationMatrix]),
    clip_before: bool,
    (window, cx): (&mut Window, &mut App),
    paint: impl FnOnce(&mut Window, &mut App),
) {
    let scale = window.scale_factor();
    // Одна запись на сегмент: второе размещение того же сегмента повторило бы его.
    let first = |i: usize| {
        !list[..i]
            .iter()
            .any(|q| q.placement.id == list[i].placement.id)
    };
    let list: Vec<(Piece, TransformationMatrix)> = (0..list.len().min(matrices.len()))
        .filter(|&i| first(i))
        .map(|i| (list[i], matrices[i]))
        .collect();
    let routes: Rc<[PaintSegment]> = list.iter().map(|(p, _)| p.placement).collect();
    let extent = list
        .iter()
        .map(|(p, _)| p)
        .map(|p| p.clip.union(&p.slice))
        .reduce(|a, b| a.union(&b));
    let Some(extent) = extent else { return };
    let before: Rc<[PaintSegmentSlice]> = list
        .iter()
        .map(|(p, _)| PaintSegmentSlice {
            placement: p.placement,
            source_clip: p.clip,
        })
        .collect();
    let recorded = crate::layout::fragment::line_admission::outside(window, |window| {
        window.record_paint_segments(extent, routes, cx, |window, cx| {
            if clip_before {
                let _ = window.with_paint_segment_slices(before, |window| paint(window, cx));
            } else {
                paint(window, cx);
            }
        })
    });
    let Ok((_, records)) = recorded else { return };
    for (p, device) in &list {
        let Some(record) = records.get(p.placement.id) else {
            continue;
        };
        let presented = presentation(*device, p.delta(), scale);
        let delta = p.delta();
        // Срез после преобразования — та же маска куска, что у копии; срез до него
        // переходит вместе с содержимым, и маска куска расширяется на его образ.
        let area = if clip_before {
            let area = image(presented, Bounds::new(p.clip.origin + delta, p.clip.size));
            pieces.reach(p.placement.id, area);
            area
        } else {
            Bounds::new(p.slice.origin + delta, p.slice.size)
        };
        let ordinal = pieces.ordinal.get();
        pieces.ordinal.set(ordinal + 1);
        let route = PaintSegment {
            id: p.placement.id,
            ordinal,
            source: area.origin,
            destination: area.origin,
        };
        let slice = PaintSegmentSlice {
            placement: route,
            source_clip: area,
        };
        let plan = PaintPlan::record(record.clone());
        let _ = window.with_paint_segment_slices(Rc::from([slice]), |window| {
            window.with_paint_presentation(presented, |window| {
                window.paint_plan(
                    &plan,
                    PaintPlacement {
                        source: point(px(0.), px(0.)),
                        destination: point(px(0.), px(0.)),
                        clip: area,
                    },
                )
            })
        });
    }
}

/// Образ прямоугольника под осевой матрицей (точки CSS).
fn image(m: TransformationMatrix, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let p = m.apply(b.origin);
    let q = m.apply(point(b.right(), b.bottom()));
    Bounds {
        origin: point(p.x.min(q.x), p.y.min(q.y)),
        size: size((p.x - q.x).abs(), (p.y - q.y).abs()),
    }
}

/// Матрица источника (точки устройства) на месте куска, сдвинутого на `delta`, в точках CSS.
fn presentation(
    m: TransformationMatrix,
    delta: gpui::Point<Pixels>,
    scale: f32,
) -> TransformationMatrix {
    let (dx, dy) = (f32::from(delta.x) * scale, f32::from(delta.y) * scale);
    let [[a, b], [c, d]] = m.rotation_scale;
    TransformationMatrix {
        rotation_scale: m.rotation_scale,
        translation: [
            (m.translation[0] + dx - a * dx - b * dy) / scale,
            (m.translation[1] + dy - c * dx - d * dy) / scale,
        ],
    }
}
