//! Кусок разрезанного ребёнка, каким его видят художники фрагмента в записи главной копии.

use gpui::{Bounds, PaintSegment, Pixels, point, px, size};

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
    /// Пустой хвост фрагментаинера за разрезом, куда тянутся дорожки сетки
    /// (`gap_fragment::Scope::end`).
    pub tail: Pixels,
    /// Срез узла со своим допуском (`Pieces::current`), а не кусок ребёнка.
    pub admitted: bool,
    /// Собственное окно узла обрезает низ исходного среза фрагмента.
    pub end_clipped: bool,
}

impl Piece {
    /// Собственное окно узла сохраняет геометрию фрагмента, но может раньше
    /// обрезать его краску. Края сравниваются на одной сетке устройства.
    pub(crate) fn with_admission(&self, own: &gpui::PaintSegmentSlice) -> Self {
        let shift = (own.placement.destination - own.placement.source) - self.delta();
        let end_clipped = own.source_clip.bottom() + shift.y
            < self.slice.bottom() - px(crate::tolerance::COORD_EPS);
        Self {
            placement: own.placement,
            fragment: Bounds::new(self.fragment.origin - shift, self.fragment.size),
            clip: own.source_clip,
            slice: own.source_clip,
            admitted: true,
            end_clipped,
            ..*self
        }
    }

    /// Геометрия линейки уже ограничена фрагментом. Её пустой хвост допускается
    /// у края исходного среза; внутренний cut окна и consumed top сохраняются.
    pub(crate) fn gap_slice(&self, end: Pixels) -> Bounds<Pixels> {
        let reach = if self.admitted {
            px(0.)
        } else {
            self.slice.size.width
        };
        let mut bottom = self.slice.bottom() + reach;
        if self.admitted && !self.end_clipped && end > self.fragment.bottom() {
            bottom = bottom.max(end);
        }
        Bounds::from_corners(
            point(self.slice.left(), self.slice.top() - reach),
            point(self.slice.right(), bottom),
        )
    }

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

    pub(crate) fn delta(&self) -> gpui::Point<Pixels> {
        self.placement.destination - self.placement.source
    }

    /// Есть ли в этом фрагменте сама коробка (а не только её переполнение).
    pub(crate) fn holds(&self, b: Bounds<Pixels>) -> bool {
        let (top, bottom) = (self.fragment.top(), self.fragment.bottom());
        if b.size.height <= px(0.01) {
            return b.top() >= top - px(0.01) && b.top() < bottom - px(0.01);
        }
        b.top() < bottom - px(0.01) && b.bottom() > top + px(0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{PaintSegmentId, PaintSegmentSlice};

    fn piece() -> Piece {
        let fragment = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(90.2)));
        let slice = Bounds::new(fragment.origin, size(px(100.), px(90.)));
        Piece {
            placement: PaintSegment {
                id: PaintSegmentId(0),
                ordinal: 0,
                source: point(px(0.), px(0.)),
                destination: point(px(0.), px(0.)),
            },
            fragment,
            clip: slice,
            slice,
            tail: px(10.),
            admitted: false,
            end_clipped: false,
        }
    }

    fn admit(p: &Piece, top: f32, bottom: f32, shift: f32) -> Piece {
        let mut placement = p.placement;
        placement.destination.y += px(shift);
        p.with_admission(&PaintSegmentSlice {
            placement,
            source_clip: Bounds::from_corners(point(px(0.), px(top)), point(px(100.), px(bottom))),
        })
    }

    #[test]
    fn gap_ink_reaches_tail_beyond_snapped_fragment_edge() {
        let p = admit(&piece(), 0., 90., 0.);
        let clip = p.gap_slice(p.fragment.bottom() + p.tail);
        assert_eq!(clip.bottom(), px(100.2));
        assert_eq!(clip.top(), px(0.));
    }

    #[test]
    fn internal_item_window_still_clips_tail_ink() {
        let p = admit(&piece(), 0., 80., 0.);
        assert_eq!(p.gap_slice(p.fragment.bottom() + p.tail), p.slice);
        assert_eq!(p.tail, px(10.));
    }

    #[test]
    fn consumed_top_is_preserved_when_bottom_reaches_tail() {
        let p = admit(&piece(), 20., 90., 0.);
        let clip = p.gap_slice(p.fragment.bottom() + p.tail);
        assert_eq!(clip.top(), px(20.));
        assert_eq!(clip.bottom(), px(100.2));
    }

    #[test]
    fn shifted_item_compares_edges_in_fragment_coordinates() {
        let p = admit(&piece(), -30., 60., 30.);
        let clip = p.gap_slice(p.fragment.bottom() + p.tail);
        assert_eq!(clip.top(), px(-30.));
        assert_eq!(clip.bottom(), p.fragment.bottom() + px(10.));
        let cut = admit(&piece(), -30., 59., 30.);
        assert_eq!(cut.gap_slice(cut.fragment.bottom() + cut.tail), cut.slice);
    }

    #[test]
    fn rule_without_tail_keeps_its_admission_slice() {
        let p = admit(&piece(), 0., 90., 0.);
        assert_eq!(p.gap_slice(p.fragment.bottom()), p.slice);
    }
}
