//! Допуск строки в записи главной копии разрезанного ребёнка стопки. Строка неделима
//! (css-break-3 §4.4), и строка, начатая в куске и перешедшая его конец (строка выше
//! фрагментаинера или ряда вложенного многоколоночника, чей конец — внешний разрез),
//! рисуется только в куске своего начала: его срезом, продолженным вниз до конца строки
//! и вылета, — как монолит-блок (`element::admission`). Blink: строка переполняет
//! фрагментаинер своего начала и в следующий не переходит. Атомы строки идут с ней.

use crate::layout::fragment::paginate::watch::Watched;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, PaintSegmentSlice, Pixels, Window, point};
use std::cell::RefCell;
use std::rc::Rc;

/// Куски записи: срез и фрагмент по блочной оси (координаты источника — окна записи), и
/// расширения масок воспроизведения, найденные при краске строк.
pub(crate) struct Lines {
    pieces: Rc<[(PaintSegmentSlice, (f32, f32))]>,
    watched: Option<Rc<Watched>>,
    spill: Pixels,
    extra: RefCell<Vec<(u64, Bounds<Pixels>)>>,
}

impl Lines {
    pub(crate) fn new(
        pieces: Rc<[(PaintSegmentSlice, (f32, f32))]>,
        watched: Option<Rc<Watched>>,
        spill: Pixels,
    ) -> Rc<Self> {
        Rc::new(Self {
            pieces,
            watched,
            spill,
            extra: RefCell::default(),
        })
    }

    /// Маски кусков (номер сегмента, границы в координатах окна), продлённые строками.
    pub(crate) fn take_extra(&self) -> Vec<(u64, Bounds<Pixels>)> {
        self.extra.take()
    }

    /// Срез куска начала строки `[top, bottom)` выше фрагмента этого куска, перешедшей его
    /// конец, продлённый вниз. Обычная строка у разреза не допускается: её место решила
    /// раскладка куска. Внутри допущенного узла (монолит, окно элемента) действует его срез.
    fn admit(&self, top: f32, bottom: f32) -> Option<PaintSegmentSlice> {
        if (self.watched.as_ref()).is_some_and(|w| w.current_admitted().is_some()) {
            return None;
        }
        let (s, _) = self.pieces.iter().find(|(_, (from, end))| {
            (from - COORD_EPS..end - COORD_EPS).contains(&top)
                && bottom > end + COORD_EPS
                && bottom - top > end - from + COORD_EPS
        })?;
        let clip = s.source_clip;
        let source_clip = Bounds::from_corners(
            clip.origin,
            point(clip.right(), Pixels::from(bottom) + self.spill),
        );
        let delta = s.placement.destination - s.placement.source;
        let placed = Bounds::new(source_clip.origin + delta, source_clip.size);
        self.extra.borrow_mut().push((s.placement.id.0, placed));
        Some(PaintSegmentSlice {
            placement: s.placement,
            source_clip,
        })
    }
}

/// Запись `record` главной копии, чьи строки допускаются по кускам `lines`.
pub(crate) fn with<R>(
    lines: Rc<Lines>,
    window: &mut Window,
    record: impl FnOnce(&mut Window) -> R,
) -> R {
    window.with_render_context(Some(lines), record)
}

/// Вложенная запись (свои сегменты): строки внутри неё куски внешней записи не видят.
pub(crate) fn outside<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    window.with_render_context::<Lines, R>(None, f)
}

/// Краска строки `[top, bottom)` (координаты окна): в записи главной копии строка,
/// перешедшая конец куска своего начала, рисуется только в нём.
pub(crate) fn paint<R>(
    window: &mut Window,
    (top, bottom): (Pixels, Pixels),
    paint: impl FnOnce(&mut Window) -> R,
) -> R {
    let slice = (window.render_context::<Lines>())
        .and_then(|lines| lines.admit(f32::from(top), f32::from(bottom)));
    let Some(slice) = slice else {
        return paint(window);
    };
    let mut paint = Some(paint);
    match window.with_paint_segment_slices(Rc::from([slice]), |w| (paint.take().expect("line"))(w))
    {
        Ok(result) => result,
        // Сегменты чужой записи: строка на своём месте.
        Err(_) => (paint.take().expect("unpainted line"))(window),
    }
}
