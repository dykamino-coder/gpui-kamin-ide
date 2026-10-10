//! План размещения по якорю: вычисление вставок, запасные позиции, переворот сторон.

use super::resolve::last_named;
use super::settle::DefaultAnchor;
use super::{AnchorPlan, AnchorRec, CB_PARENT, IMPLICIT, LAST_IMPLICIT, NAMED, tf_under};
use crate::style::computed::Computed;
use crate::style::values::value::{AnchorFn, AnchorSide, Len, anchor_get};
use gpui::{Bounds, Pixels};
mod axis;
mod tactics;
use axis::Edge;
mod align;
mod of;

/// Как `IsFlippedX`/`IsFlippedY` у Blink: x перевёрнут при горизонтальном
/// rtl и при vertical-rl, y — при вертикальном письме с `direction: rtl`.
pub(super) fn flipped(c: &Computed) -> (bool, bool) {
    let vertical = c.vertical == Some(true);
    let rtl = c.rtl == Some(true);
    let x = if vertical {
        c.vertical_rl == Some(true)
    } else {
        rtl
    };
    (x, vertical && rtl)
}

impl AnchorPlan {
    /// Запись якоря по имени (или якоря по умолчанию) — только приемлемого.
    pub(super) fn rec(&self, name: Option<&str>) -> Option<AnchorRec> {
        // Реестр ТЕКУЩЕГО кадра знает только якоря, уже прошедшие подготовку.
        // Якорь раньше по дереву, но в слое, который готовится ПОЗЖЕ коробки
        // (слой ICB у `fixed`, `interact::icb_close` — последние дети
        // документа), в нём ещё не записан: `anchor-abspos-to-fixedpos-001` —
        // `fixed`-якорь и абсолют без краёв в потоке, коробка падала на
        // статическую позицию под якорь. Спека требует лишь «laid out
        // strictly before» по дереву (§target) — его и проверяет фильтр `seq`
        // у записи прошлого кадра (стенд ждёт устоявшихся кадров, как у
        // `anchor-size()`). Якорь ниже по дереву (`anchor-position-circular`)
        // фильтр отсекает.
        let named = |n: &str| {
            NAMED
                .with(|m| m.borrow().get(n).copied())
                .or_else(|| last_named(n, self.seq))
        };
        let r = match name {
            Some(n) => named(n),
            None => match &self.default_anchor {
                Some(DefaultAnchor::Named(n)) => named(n.as_str()),
                Some(DefaultAnchor::Implicit(id)) => IMPLICIT
                    .with(|m| m.borrow().get(id).copied())
                    .or_else(|| LAST_IMPLICIT.with(|m| m.borrow().get(id).copied())),
                None => None,
            },
        }?;
        self.acceptable(&r).then_some(r)
    }

    /// §target, acceptable anchor element: «same original containing block»
    /// либо элемент, порождающий содержащий блок якоря, сам приемлем — то
    /// есть содержащий блок коробки лежит в цепочке содержащих блоков
    /// якоря (или сам является якорем: неявный якорь позиционированного
    /// хозяина). Начальный содержащий блок (окно, `fixed`) принимает всех;
    /// звено без пробы (строчный или атомарный содержащий блок) — цепочка
    /// неизвестна, якорь не отвергается (`no-anchor-anchor-center`).
    fn acceptable(&self, r: &AnchorRec) -> bool {
        if self.fixed || self.cb_node == 0 || r.id == self.cb_node {
            return true;
        }
        let mut n = r.cb;
        for _ in 0..64 {
            if n == self.cb_node {
                return true;
            }
            if n == 0 {
                return false;
            }
            match CB_PARENT.with(|m| m.borrow().get(&n).copied()) {
                Some(p) => n = p,
                None => return true,
            }
        }
        true
    }

    /// Рамка якоря в системе координат коробки: если самый внутренний
    /// трансформ якоря — предок и самой коробки, вся цепочка трансформов у
    /// них общая и сравнивать надо до-трансформные рамки; иначе — рамку
    /// после трансформов (§2, «in the coordinate space of the absolutely
    /// positioned element's containing block»). Частично общая цепочка —
    /// приближение: берётся отображённая.
    fn lookup(&self, name: Option<&str>) -> Option<Bounds<Pixels>> {
        self.rec(name).map(|r| {
            if r.tf_top == 0 || tf_under(r.tf_top) {
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
    fn resolve_anchor(&self, first: AnchorFn, y_axis: bool, end_side: bool) -> Edge {
        let mut f = Some(first);
        // Цепочка запасных значений: `anchor(top, anchor(--a1 bottom))`.
        for _ in 0..4 {
            let Some(cur) = f.take() else { break };
            let hit = self.hit_of(&cur, y_axis, end_side);
            match (hit, cur.fallback) {
                (Some(want), _) => return Edge::Abs(want),
                (None, Some(Len::Px(v))) => return Edge::FromEdge(v),
                (None, Some(Len::Anchor(j))) => f = anchor_get(j),
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
