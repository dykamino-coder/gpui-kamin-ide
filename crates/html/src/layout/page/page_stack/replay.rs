//! Листы печати рисуют куски ребёнка записью его главной копии (`layout::fragment::replay`):
//! главная копия пишется в несдвинутых координатах стопки до матрицы листа, каждый кусок
//! играет свой сегмент под матрицей своего листа (`with_paint_presentation`, как нативные
//! записи OOF-продолжений) и маской своего фрагмента — той же, что у копии. Копия под
//! матрицей снимает края на сетку устройства ПОСЛЕ масштаба стопки, запись — до него:
//! равны они только при масштабе 1 и переносе на целые пиксели устройства. На копиях
//! остаются также: листы с поворотом, полосы повтора таблиц, вертикальная ось корня,
//! листы разной ширины page area, копии со своей мерой или продолжением потока и дети с
//! внепоточными источниками.

use super::PageStack;
use crate::layout::fragment::axis_slice::snapped;
use crate::layout::fragment::replay::{PieceGeometry, Replayed, Routes, Source};
use crate::layout::fragment::types::Frag;
use gpui::{App, Bounds, Pixels, Point, Window, point, px, size};
use std::collections::HashMap;
mod play;

/// Записи главных копий по номеру ребёнка; `None` — запись не удалась, куски не рисуются.
pub(super) type Played = HashMap<usize, Option<Replayed>>;

/// Коробка `b` в несдвинутых координатах стопки — в координатах окна (масштаб `s` вокруг
/// левого верха стопки, как `PageStack::area_mask`).
fn present(bounds: Bounds<Pixels>, s: f32, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let o = bounds.origin;
    Bounds::new(
        point(o.x + (b.origin.x - o.x) * s, o.y + (b.origin.y - o.y) * s),
        size(b.size.width * s, b.size.height * s),
    )
}

/// Обратное к [`present`].
fn unpresent(bounds: Bounds<Pixels>, s: f32, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let o = bounds.origin;
    let at = |v: Pixels, o: Pixels| o + px(f32::from(v - o) / s);
    Bounds::new(
        point(at(b.origin.x, o.x), at(b.origin.y, o.y)),
        size(
            px(f32::from(b.size.width) / s),
            px(f32::from(b.size.height) / s),
        ),
    )
}

/// Геометрия кусков на листах одного кадра стопки.
struct Sheets<'a> {
    stack: &'a PageStack,
    bounds: Bounds<Pixels>,
    s: f32,
    scale: f32,
}

impl PieceGeometry for Sheets<'_> {
    fn piece_at(&self, f: &Frag) -> Point<Pixels> {
        let (sx, sy) = self.stack.sheet_origin(f.col);
        let (ax, ay) = self.stack.geom(f.col).area_origin();
        let top = self.stack.kids[f.kid].inner_top;
        let o = self.bounds.origin;
        point(o.x + px(sx + ax), o.y + px(sy + ay + f.y - top))
    }

    fn piece_clip(&self, f: &Frag, _at: Point<Pixels>) -> (Bounds<Pixels>, Bounds<Pixels>) {
        let page = self.stack.area_mask(self.bounds, f.col);
        let mask = self.stack.piece_mask(self.bounds, f, page, self.scale);
        let mask = unpresent(self.bounds, self.s, mask.bounds);
        (mask, mask)
    }

    fn fragment_width(&self, f: &Frag) -> f32 {
        self.stack.geom(f.col).area.0
    }

    fn slot_area(&self, _f: &Frag, _clip: Bounds<Pixels>, mask: Bounds<Pixels>) -> Bounds<Pixels> {
        mask
    }

    fn exact_shift(&self, delta: Point<Pixels>) -> bool {
        snapped(delta, self.scale) == delta
    }
}

impl PageStack {
    /// Ребёнок, чьи куски рисует запись главной копии (решение одно для prepaint и paint).
    fn replays(&self, kid: usize, plan: &[Frag]) -> bool {
        let k = &self.kids[kid];
        if self.axis.is_vertical() || !k.replay || k.leaf.is_some() || k.repeat.is_some() {
            return false;
        }
        let Some(main) = plan.iter().find(|f| f.kid == kid && f.copy == 0) else {
            return false;
        };
        let width = self.geom(main.col).area.0;
        let own = |f: &&Frag| f.kid == kid;
        plan.iter().filter(own).count() > 1
            && k.oof.is_none()
            && plan.iter().filter(own).all(|f| {
                let g = self.geom(f.col);
                g.turn.is_multiple_of(2)
                    && g.area.0 == width
                    && self.slot(f.col).is_some()
                    && f.copy <= k.frags.len()
                    && (f.copy == 0
                        || !(self.continuations.contains_key(&(kid, f.copy))
                            || self.measured.contains_key(&(kid, f.copy))))
            })
    }

    /// Размещения кусков ребёнка `kid` на листах; `None` — куски рисуют копии.
    pub(super) fn page_routes(
        &self,
        kid: usize,
        plan: &[Frag],
        bounds: Bounds<Pixels>,
        window: &Window,
    ) -> Option<Routes> {
        if !self.replays(kid, plan) || self.grid.get().1 != 1.0 {
            return None;
        }
        let geometry = Sheets {
            stack: self,
            bounds,
            s: self.grid.get().1,
            scale: window.scale_factor(),
        };
        let k = &self.kids[kid];
        let source = Source {
            monoliths: false,
            regrow: k.regrow.as_ref(),
            watched: k.watched.as_ref(),
            effects: false,
            clone_root: None,
        };
        let spill = window.viewport_size().width.max(bounds.size.width);
        Routes::new(kid, plan, &geometry, source, spill)
    }

    /// Записи главных копий — до матрицы листов, в несдвинутых координатах стопки.
    pub(super) fn record_replays(
        &mut self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        window: &mut Window,
        cx: &mut App,
    ) -> Played {
        let mut out = HashMap::new();
        for kid in plan.iter().filter(|f| f.copy == 0).map(|f| f.kid) {
            let Some(routes) = self.page_routes(kid, plan, bounds, window) else {
                continue;
            };
            let records = self.copy_records();
            let el = (&mut self.kids[kid].el, None, records);
            out.insert(kid, routes.record(el, window, cx).ok());
        }
        out
    }
}
