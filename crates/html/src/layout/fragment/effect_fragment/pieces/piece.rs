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
