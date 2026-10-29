//! Художники, читающие границы фрагмента, в записи главной копии разрезанного ребёнка видят
//! все его куски (сегменты записи с размещениями, фрагментами и срезами в координатах
//! источника), а не один фрагмент `Scope` копии. Тень режется собственным допуском узла по
//! кускам, где коробка есть, с продлением на её настоящих краях (css-break-3 §5.4);
//! преобразование — вложенной записью, возвращаемой в каждый кусок своей матрицей вокруг
//! опорной коробки его фрагмента (§5.5, presentation scope P08). Куска без коробки её
//! краска не достигает (Blink: там нет её фрагмента).

use crate::layout::cx::deferred::Deferred;
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintSegment, PaintSegmentId, PaintSegmentSlice,
    Pixels, TransformationMatrix, Window, point, px, size,
};
use std::cell::Cell;
use std::rc::Rc;

/// Кусок разрезанного ребёнка в координатах источника (главной копии).
#[derive(Clone, Copy)]
pub(crate) struct Piece {
    /// Размещение главной копии в сегменте куска.
    pub placement: PaintSegment,
    /// Границы фрагмента: ширина колонки, высота куска.
    pub fragment: Bounds<Pixels>,
    /// Срез куска до привязки к пикселям устройства.
    pub clip: Bounds<Pixels>,
    /// Срез куска, снятый на пиксели устройства (маска копии).
    pub slice: Bounds<Pixels>,
}

impl Piece {
    /// Опорная коробка `b` в этом фрагменте (как `super::reference_box` у копии).
    pub(crate) fn reference(&self, b: Bounds<Pixels>) -> Bounds<Pixels> {
        let start = b.top().max(self.fragment.top());
        let end = b.bottom().min(self.fragment.bottom());
        Bounds {
            origin: point(
                b.left(),
                if end <= start {
                    self.fragment.top()
                } else {
                    start
                },
            ),
            size: size(b.size.width, (end - start).max(px(0.0))),
        }
    }

    fn delta(&self) -> gpui::Point<Pixels> {
        self.placement.destination - self.placement.source
    }

    /// Есть ли в этом фрагменте сама коробка (а не только её переполнение).
    fn holds(&self, b: Bounds<Pixels>) -> bool {
        let (top, bottom) = (self.fragment.top(), self.fragment.bottom());
        if b.size.height <= px(0.01) {
            return b.top() >= top - px(0.01) && b.top() < bottom - px(0.01);
        }
        b.top() < bottom - px(0.01) && b.bottom() > top + px(0.01)
    }
}

/// Куски ребёнка на время записи его главной копии.
pub(crate) struct Pieces {
    pub pieces: Rc<[Piece]>,
    /// Области за срезами кусков, куда рисуют художники фрагмента (по сегменту, в
    /// координатах окна): маска воспроизведения куска расширяется на них.
    pub extra: Cell<Vec<(u64, Bounds<Pixels>)>>,
    /// Следующий порядковый номер размещения возвращённой вложенной записи.
    ordinal: Cell<u32>,
}

/// Порядковые номера возвращённых записей — выше размещений кусков и окон элементов.
const REENTRY: u32 = 1 << 30;

impl Pieces {
    pub(crate) fn new(pieces: Rc<[Piece]>) -> Self {
        Self {
            pieces,
            extra: Default::default(),
            ordinal: Cell::new(REENTRY),
        }
    }

    fn reach(&self, segment: PaintSegmentId, placed: Bounds<Pixels>) {
        let mut all = self.extra.take();
        all.push((segment.0, placed));
        self.extra.set(all);
    }
}

pub(crate) fn with<R>(owner: &Deferred, pieces: Option<Rc<Pieces>>, f: impl FnOnce() -> R) -> R {
    let previous = owner.pieces.replace(pieces);
    let result = f();
    owner.pieces.replace(previous);
    result
}

pub(crate) fn active(owner: Option<&Deferred>) -> Option<Rc<Pieces>> {
    owner?.pieces.borrow().clone()
}

/// Резкая наружная тень коробки `bounds` с чернилами `ink` (верх, низ) по кускам: срез
/// куска, где коробка есть, продлённый на чернила у её настоящих краёв.
pub(crate) fn paint_shadow(
    pieces: &Pieces,
    bounds: Bounds<Pixels>,
    ink: (Pixels, Pixels),
    window: &mut Window,
    f: impl FnOnce(&mut Window),
) {
    let scale = window.scale_factor();
    let mut slices = Vec::new();
    for p in pieces.pieces.iter().filter(|p| p.holds(bounds)) {
        let mut top = p.slice.top();
        let mut bottom = p.slice.bottom();
        if bounds.top() >= p.fragment.top() - px(0.01) {
            top = top.min(ink.0);
        }
        if bounds.bottom() <= p.fragment.bottom() + px(0.01) {
            bottom = bottom.max(ink.1);
        }
        let clip = Bounds {
            origin: point(p.slice.left(), top),
            size: size(p.slice.size.width, bottom - top),
        };
        let source_clip = super::super::fragment_mask::snap(clip, scale).bounds;
        pieces.reach(
            p.placement.id,
            Bounds::new(source_clip.origin + p.delta(), source_clip.size),
        );
        slices.push(PaintSegmentSlice {
            placement: p.placement,
            source_clip,
        });
    }
    if slices.is_empty() {
        return;
    }
    let _ = window.with_paint_segment_slices(slices.into(), f);
}

/// Плоское преобразование коробки `bounds` по кускам. Поддерево пишется один раз
/// вложенной записью по сегментам кусков (уже на месте каждого куска); каждый сегмент
/// возвращается в запись главной копии под своей матрицей `matrix(reference)` вокруг
/// опорной коробки своего фрагмента (`matrices` — по кускам, в точках устройства, в
/// координатах источника). Как у копии: при осевой матрице без обрезки предка срез куска стоит до
/// преобразования, иначе — после него, маской куска.
pub(crate) fn paint_transformed(
    pieces: &Pieces,
    matrices: &[TransformationMatrix],
    clip_before: bool,
    (window, cx): (&mut Window, &mut App),
    paint: impl FnOnce(&mut Window, &mut App),
) {
    let scale = window.scale_factor();
    let routes: Rc<[PaintSegment]> = pieces.pieces.iter().map(|p| p.placement).collect();
    let extent = pieces
        .pieces
        .iter()
        .map(|p| p.clip.union(&p.slice))
        .reduce(|a, b| a.union(&b));
    let Some(extent) = extent else { return };
    let before: Rc<[PaintSegmentSlice]> = pieces
        .pieces
        .iter()
        .map(|p| PaintSegmentSlice {
            placement: p.placement,
            source_clip: p.clip,
        })
        .collect();
    let recorded = window.record_paint_segments(extent, routes, cx, |window, cx| {
        if clip_before {
            let _ = window.with_paint_segment_slices(before, |window| paint(window, cx));
        } else {
            paint(window, cx);
        }
    });
    let Ok((_, records)) = recorded else { return };
    for (p, &device) in pieces.pieces.iter().zip(matrices) {
        let Some(record) = records.get(p.placement.id) else {
            continue;
        };
        let presented = presentation(device, p.delta(), scale);
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
