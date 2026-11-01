//! Художники, читающие границы фрагмента, в записи главной копии разрезанного ребёнка видят
//! все его куски (сегменты записи с размещениями, фрагментами и срезами в координатах
//! источника), а не один фрагмент `Scope` копии. Тень режется собственным допуском узла по
//! кускам, где коробка есть, с продлением на её настоящих краях (css-break-3 §5.4);
//! преобразование — вложенной записью, возвращаемой в каждый кусок своей матрицей вокруг
//! опорной коробки его фрагмента (§5.5, presentation scope P08). Куска без коробки её
//! краска не достигает (Blink: там нет её фрагмента).

use crate::layout::cx::deferred::Deferred;
use gpui::{Bounds, PaintSegmentId, PaintSegmentSlice, Pixels, Window, point, px, size};
use std::cell::Cell;
mod piece;
mod transform;
pub(crate) use piece::Piece;
use std::rc::Rc;
pub(crate) use transform::paint_transformed;

/// Куски ребёнка на время записи его главной копии.
pub(crate) struct Pieces {
    pub pieces: Rc<[Piece]>,
    /// Области за срезами кусков, куда рисуют художники фрагмента (по сегменту, в
    /// координатах окна): маска воспроизведения куска расширяется на них.
    pub extra: Cell<Vec<(u64, Bounds<Pixels>)>>,
    /// Следующий порядковый номер размещения возвращённой вложенной записи.
    ordinal: Cell<u32>,
    /// Допуск узлов главной копии: внутри допущенного узла куски — его срезы.
    watched: Option<Rc<crate::layout::fragment::paginate::watch::Watched>>,
}

/// Порядковые номера возвращённых записей — выше размещений кусков и окон элементов.
const REENTRY: u32 = 1 << 30;

impl Pieces {
    pub(crate) fn new(
        pieces: Rc<[Piece]>,
        watched: Option<Rc<crate::layout::fragment::paginate::watch::Watched>>,
    ) -> Self {
        Self {
            pieces,
            extra: Default::default(),
            ordinal: Cell::new(REENTRY),
            watched,
        }
    }

    /// Куски, которые видит художник сейчас: внутри узла с собственным допуском — его
    /// срезы (фрагмент куска перенесён на сдвиг окна), иначе — куски ребёнка.
    pub(crate) fn current(&self) -> Rc<[Piece]> {
        let Some(own) = self.watched.as_ref().and_then(|w| w.current_admitted()) else {
            return self.pieces.clone();
        };
        own.iter()
            .filter_map(|s| {
                let p = self
                    .pieces
                    .iter()
                    .find(|p| p.placement.id == s.placement.id)?;
                Some(p.with_admission(s))
            })
            .collect()
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
    for p in pieces.current().iter().filter(|p| p.holds(bounds)) {
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

/// Краска украшений, уже ограниченных геометрией фрагмента: допуск сохраняет
/// внутренний cut узла и допускает хвост у края фрагмента; обрезки предков действуют.
pub(crate) fn paint_unclipped(
    pieces: &Pieces,
    p: &Piece,
    end: Pixels,
    window: &mut Window,
    f: impl FnOnce(&mut Window),
) {
    let source_clip = p.gap_slice(end);
    pieces.reach(
        p.placement.id,
        Bounds::new(source_clip.origin + p.delta(), source_clip.size),
    );
    let slice = PaintSegmentSlice {
        placement: p.placement,
        source_clip,
    };
    let _ = window.with_paint_segment_slices(Rc::from([slice]), f);
}
