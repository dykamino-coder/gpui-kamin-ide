//! План размещения по якорю: вычисление вставок, запасные позиции, переворот сторон.

use super::{AnchorPlan, tf_under};
use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::{AnchorFn, AnchorSide, Len, anchor_get};
use gpui::{Bounds, Pixels};
mod axis;
mod compute;
use axis::Edge;
mod align;
mod lookup;
mod of;

/// Как `IsFlippedX`/`IsFlippedY` у Blink: x перевёрнут при горизонтальном
/// rtl и при vertical-rl, y — при вертикальном письме с `direction: rtl`.
pub(super) fn flipped(c: &Computed) -> (bool, bool) {
    let vertical = c.wm().vertical();
    let rtl = c.wm().rtl();
    let x = if vertical { c.wm().block_rl() } else { rtl };
    (x, vertical && rtl)
}

impl AnchorPlan {
    /// Рамка якоря в системе координат коробки: если самый внутренний
    /// трансформ якоря — предок и самой коробки, вся цепочка трансформов у
    /// них общая и сравнивать надо до-трансформные рамки; иначе — рамку
    /// после трансформов (§2, «in the coordinate space of the absolutely
    /// positioned element's containing block»). Частично общая цепочка —
    /// приближение: берётся отображённая.
    fn lookup(&self, name: Option<&str>) -> Option<Bounds<Pixels>> {
        self.rec(name).map(|r| {
            if r.tf_top == 0 || tf_under(r.tf_top, self.values.vcx()) {
                r.rect
            } else {
                r.rect_tf
            }
        })
    }

    /// Доля [0;1] вдоль физической оси от её начала, куда указывает
    /// `<anchor-side>`; `None` — сторона другой оси (функция неразрешима).
    /// Таблица — как `ResolveAnchorValue` у Blink.
    fn fraction(&self, side: AnchorSide, y_axis: bool, end_side: bool) -> Option<f32> {
        let cb = if y_axis {
            self.cb_flipped.1
        } else {
            self.cb_flipped.0
        };
        let own = if y_axis {
            self.own_flipped.1
        } else {
            self.own_flipped.0
        };
        let flip = |f: bool, t: f32| if f { 1.0 - t } else { t };
        Some(match side {
            AnchorSide::Top if y_axis => 0.0,
            AnchorSide::Bottom if y_axis => 1.0,
            AnchorSide::Left if !y_axis => 0.0,
            AnchorSide::Right if !y_axis => 1.0,
            AnchorSide::Top | AnchorSide::Bottom | AnchorSide::Left | AnchorSide::Right => {
                return None;
            }
            AnchorSide::Inside => {
                if end_side {
                    1.0
                } else {
                    0.0
                }
            }
            AnchorSide::Outside => {
                if end_side {
                    0.0
                } else {
                    1.0
                }
            }
            AnchorSide::Start => flip(cb, 0.0),
            AnchorSide::End => flip(cb, 1.0),
            AnchorSide::SelfStart => flip(own, 0.0),
            AnchorSide::SelfEnd => flip(own, 1.0),
            AnchorSide::Pct(p) => flip(cb, p),
        })
    }

    /// Экранная координата края по одной `anchor()` (без запасного значения).
    fn hit_one(&self, f: &AnchorFn, y_axis: bool, end_side: bool) -> Option<f32> {
        let a = self.lookup(f.name.as_deref())?;
        let t = self.fraction(f.side, y_axis, end_side)?;
        let (start, len) = if y_axis {
            (f32::from(a.origin.y), f32::from(a.size.height))
        } else {
            (f32::from(a.origin.x), f32::from(a.size.width))
        };
        Some(start + len * t + f.add)
    }

    /// То же для `min()`/`max()` (`AnchorFn::alts`): все доводы в одной
    /// экранной системе, берётся меньший/больший; неразрешимый довод делает
    /// неразрешимой всю функцию — дальше её запасное значение.
    fn hit_of(&self, f: &AnchorFn, y_axis: bool, end_side: bool) -> Option<f32> {
        let mut acc = self.hit_one(f, y_axis, end_side)?;
        for g in &f.alts {
            let v = self.hit_one(g, y_axis, end_side)?;
            acc = if f.max { acc.max(v) } else { acc.min(v) };
        }
        Some(acc)
    }

    /// Куда просится край по `anchor()`: экранная координата; вставка в
    /// точках от края содержащего блока (запасное значение); ничего.
    fn resolve_anchor(
        &self,
        first: std::rc::Rc<AnchorFn>,
        y_axis: bool,
        end_side: bool,
        vcx: ValueCx<'_>,
    ) -> Edge {
        let mut f = Some(first);
        // Цепочка запасных значений: `anchor(top, anchor(--a1 bottom))`.
        for _ in 0..4 {
            let Some(cur) = f.take() else { break };
            let hit = self.hit_of(&cur, y_axis, end_side);
            match (hit, cur.fallback) {
                (Some(want), _) => return Edge::Abs(want),
                (None, Some(Len::Px(v))) => return Edge::FromEdge(v),
                (None, Some(Len::Anchor(j))) => f = anchor_get(j, vcx),
                // Якоря нет и запаса нет: по спеке — статическая позиция,
                // но статически это было неотличимо (`settle_static` такие
                // уже снял); остаток — якорь, который есть в документе, но
                // ниже по дереву (`anchor-position-circular`).
                (None, _) => return Edge::None,
            }
        }
        Edge::None
    }
}
