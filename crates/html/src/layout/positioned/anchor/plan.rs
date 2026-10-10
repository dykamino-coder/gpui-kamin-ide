//! План размещения по якорю: вычисление вставок, запасные позиции, переворот сторон.

use super::area::{Al, Tracks, default_al, physical_area};
use super::parse::{FLIP_BLOCK, FLIP_INLINE, FLIP_START};
use super::resolve::last_named;
use super::settle::{DefaultAnchor, default_anchor_of};
use super::{
    AnchorPlan, AnchorRec, CB, CB_PARENT, IMPLICIT, LAST_IMPLICIT, NAMED, Placement, SidePlan,
    tf_under,
};
use crate::style::computed::{Align, Computed, Position};
use crate::style::values::value::{AnchorFn, AnchorSide, Len, anchor_get};
use gpui::{Bounds, Pixels, Window, px};

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
    /// План есть у абсолюта хотя бы с одной `anchor()`-вставкой, с
    /// `position-area` или `anchor-center` при якоре по умолчанию, со
    /// списком `position-try-fallbacks` либо с `position-visibility`
    /// (проверка переполнения нужна и без якоря — `no-overflow`).
    pub(super) fn of(own: &Computed, inherited: &Computed) -> Option<AnchorPlan> {
        if !matches!(
            own.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        ) {
            return None;
        }
        let m = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let margin = [
            m(own.margin.top),
            m(own.margin.right),
            m(own.margin.bottom),
            m(own.margin.left),
        ];
        let side = |l: Option<Len>, margin: f32| -> Option<SidePlan> {
            let Some(Len::Anchor(i)) = l else { return None };
            let f = anchor_get(i)?;
            // `anchor-size()` во вставке — величина, не край: сдвига не даёт.
            if f.size.is_some() {
                return None;
            }
            Some(SidePlan { f, margin })
        };
        let inset = [
            own.inset.top,
            own.inset.right,
            own.inset.bottom,
            own.inset.left,
        ];
        let sides = [
            side(inset[0], margin[0]),
            side(inset[1], margin[1]),
            side(inset[2], margin[2]),
            side(inset[3], margin[3]),
        ];
        let default_anchor = default_anchor_of(own);
        let area = own
            .position_area
            .filter(|_| default_anchor.is_some())
            .map(|a| physical_area(a, inherited, own));
        let anchor_center = own.align_self == Some(Align::AnchorCenter)
            || own.justify_self == Some(Align::AnchorCenter);
        // §position-visibility, anchor-valid: «If the box references the
        // default anchor box (e.g. using 'position-area', 'anchor()' or
        // 'anchor-size()' functions, or 'anchor-center'), but the default
        // anchor box cannot be resolved…» — ссылка это САМА запись
        // `position-area`. `area` выше уже отфильтрована якорем по умолчанию,
        // и коробка без якоря (`position-visibility-anchor-valid`, #target2:
        // `position-area: block-end`, имени нет) не пряталась никогда.
        let refs_default = own.position_area.is_some()
            || anchor_center
            || sides.iter().flatten().any(|s| s.f.name.is_none());
        if sides.iter().all(Option::is_none)
            && area.is_none()
            && !(anchor_center && default_anchor.is_some())
            && own.position_try_fallbacks.is_empty()
            && own.position_visibility == 0
        {
            return None;
        }
        Some(AnchorPlan {
            sides,
            default_anchor,
            cb_flipped: flipped(inherited),
            own_flipped: flipped(own),
            area,
            inset,
            margin,
            align_self: own.align_self,
            justify_self: own.justify_self,
            safe_align: own.align_self_safe,
            safe_justify: own.justify_self_safe,
            cb_vertical: inherited.vertical == Some(true),
            cb_node: own.cb_node,
            fixed: own.position == Some(Position::Fixed)
                && !(inherited.transform_ancestor
                    || inherited.transform.is_some()
                    || inherited.contain_layout == Some(true)
                    || inherited.contain_paint == Some(true)
                    || inherited.will_change & crate::style::computed::wc::CB_FIXED != 0),
            seq: own.anchor_seq,
            refs_default,
        })
    }

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

    /// Сдвиг по оси стороны `k` (0 top, 1 right, 2 bottom, 3 left) — без
    /// `position-area`: коробка стоит у края содержащего блока на нулевой
    /// вставке.
    fn shift(&self, k: usize, own: Bounds<Pixels>) -> f32 {
        let Some(sp) = &self.sides[k] else { return 0.0 };
        let y_axis = k.is_multiple_of(2);
        let end_side = k == 1 || k == 2;
        // Край ПОЛЕЙ коробки по этой стороне.
        let own_edge = match k {
            0 => f32::from(own.origin.y) - sp.margin,
            1 => f32::from(own.origin.x) + f32::from(own.size.width) + sp.margin,
            2 => f32::from(own.origin.y) + f32::from(own.size.height) + sp.margin,
            _ => f32::from(own.origin.x) - sp.margin,
        };
        match self.resolve_anchor(sp.f.clone(), y_axis, end_side) {
            Edge::Abs(want) => want - own_edge,
            // Запасная длина в точках — вставка от края содержащего
            // блока; раскладка уже поставила коробку на нулевую вставку.
            Edge::FromEdge(v) => {
                if end_side {
                    -v
                } else {
                    v
                }
            }
            Edge::None => 0.0,
        }
    }

    /// Край inset-modified containing block со стороны `k`: край клетки,
    /// сдвинутый авторской вставкой (`auto` → 0; доля — от клетки, она и
    /// есть содержащий блок; `anchor()` — к краю якоря, §position-area).
    fn imcb_edge(&self, k: usize, cell_edge: f32, cell_len: f32) -> f32 {
        let y_axis = k.is_multiple_of(2);
        let end_side = k == 1 || k == 2;
        let sign = if end_side { -1.0 } else { 1.0 };
        match self.inset[k] {
            Some(Len::Px(v)) => cell_edge + sign * v,
            Some(Len::Pct(p)) => cell_edge + sign * p * cell_len,
            Some(Len::Anchor(i)) => match anchor_get(i) {
                Some(f) if f.size.is_none() => match self.resolve_anchor(f, y_axis, end_side) {
                    Edge::Abs(w) => w,
                    Edge::FromEdge(v) => cell_edge + sign * v,
                    Edge::None => cell_edge,
                },
                _ => cell_edge,
            },
            _ => cell_edge,
        }
    }

    /// Выравнивание в оси, нужен ли сдвиг при переполнении и задано ли
    /// `safe` (§position-area-alignment; Blink `ComputeAlignment`): явное
    /// значение; иначе `normal` — к единственной не-`auto` вставке оси
    /// (unsafe), иначе умолчание области.
    fn align_for(&self, x_axis: bool, t: Tracks) -> (Al, bool, bool) {
        // css-align: `align-self` — блочная ось СОДЕРЖАЩЕГО БЛОКА,
        // `justify-self` — строчная.
        let (explicit, safe) = if x_axis == self.cb_vertical {
            (self.align_self, self.safe_align)
        } else {
            (self.justify_self, self.safe_justify)
        };
        if let Some(a) = explicit {
            let al = match a {
                Align::Center => Al::Center,
                Align::AnchorCenter => Al::AnchorCenter,
                Align::End => Al::End,
                Align::Stretch => Al::Stretch,
                Align::Start | Align::Baseline => Al::Start,
            };
            return (al, true, safe);
        }
        let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        let (s_set, e_set) = if x_axis {
            (set(self.inset[3]), set(self.inset[1]))
        } else {
            (set(self.inset[0]), set(self.inset[2]))
        };
        match (s_set, e_set) {
            (true, false) => (Al::Start, false, false),
            (false, true) => (Al::End, false, false),
            _ => (default_al(t), true, false),
        }
    }

    /// Рамка исходного содержащего блока: реестр `CB` по `cb_node`; корень
    /// и `fixed` — окно. Нет в реестре (блок не через `styled_div_with`,
    /// например строчный) — область не применяется.
    fn cb_bounds(&self, window: &Window) -> Option<Bounds<Pixels>> {
        if self.fixed || self.cb_node == 0 {
            return Some(Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: window.viewport_size(),
            });
        }
        CB.with(|m| m.borrow().get(&self.cb_node).copied())
    }

    /// Одна ось области: сетка → клетка → inset-modified containing block →
    /// выравнивание → зажим (`settle_axis`). Возвращает сдвиг границы
    /// коробки, длину IMCB (для растяжки и `position-try-order`) и признак
    /// переполнения IMCB коробкой полей.
    fn axis_place(
        &self,
        x: bool,
        t: Tracks,
        a: Bounds<Pixels>,
        cb: Bounds<Pixels>,
        own: Bounds<Pixels>,
    ) -> (f32, f32, bool, f32) {
        let f = f32::from;
        let (cs, ce, as_, ae, os, olen) = if x {
            (
                f(cb.origin.x),
                f(cb.origin.x) + f(cb.size.width),
                f(a.origin.x),
                f(a.origin.x) + f(a.size.width),
                f(own.origin.x),
                f(own.size.width),
            )
        } else {
            (
                f(cb.origin.y),
                f(cb.origin.y) + f(cb.size.height),
                f(a.origin.y),
                f(a.origin.y) + f(a.size.height),
                f(own.origin.y),
                f(own.size.height),
            )
        };
        // §position-area-grid-resolution: якорь за краем содержащего блока
        // раздвигает крайние линии.
        let lines = [cs.min(as_), as_, ae, ce.max(ae)];
        let (s, e) = (lines[t.lo as usize], lines[t.hi as usize]);
        let (k_start, k_end) = if x { (3usize, 1usize) } else { (0, 2) };
        let is = self.imcb_edge(k_start, s, e - s);
        let ie = self.imcb_edge(k_end, e, e - s);
        let (m_s, m_e) = (self.margin[k_start], self.margin[k_end]);
        // Выравнивается коробка ПОЛЕЙ (`auto`-поля уже нули: `m` в `of`).
        let mbox = olen + m_s + m_e;
        let (al, shift_in, safe) = self.align_for(x, t);
        let pos = match al {
            Al::Start | Al::Stretch => is,
            Al::End => ie - mbox,
            Al::Center => (is + ie - mbox) / 2.0,
            Al::AnchorCenter => (as_ + ae - mbox) / 2.0,
        };
        let start_bias = !if x {
            self.cb_flipped.0
        } else {
            self.cb_flipped.1
        };
        let pos = settle_axis(
            pos,
            mbox,
            is,
            ie,
            cs,
            ce,
            shift_in,
            start_bias,
            safe,
            al == Al::AnchorCenter,
        );
        // Четвёртое — длина самой клетки `[s, e]`: база долей (`CELL_NOW`).
        (
            pos + m_s - os,
            (ie - is).max(0.0),
            overflows(pos, mbox, is, ie),
            (e - s).max(0.0),
        )
    }

    /// Ось без `position-area`: IMCB — содержащий блок, срезанный авторскими
    /// вставками (`auto` → край блока: для `anchor-center` так велит
    /// §anchor-center, для проверки переполнения — как Blink при `auto`-конце,
    /// «растёт к краю»); `anchor-center` центрирует по якорю и зажимает
    /// (`settle_axis`), иначе коробка стоит там, куда её довезли
    /// `anchor()`-вставки (`shift`). Без рамки содержащего блока — только сдвиг.
    fn free_axis(
        &self,
        x: bool,
        cb: Option<Bounds<Pixels>>,
        own: Bounds<Pixels>,
    ) -> (f32, f32, bool) {
        let f = f32::from;
        let (k_start, k_end) = if x { (3usize, 1usize) } else { (0, 2) };
        let (os, olen) = if x {
            (f(own.origin.x), f(own.size.width))
        } else {
            (f(own.origin.y), f(own.size.height))
        };
        let (m_s, m_e) = (self.margin[k_start], self.margin[k_end]);
        let mbox = olen + m_s + m_e;
        // Без области по оси побеждает начальная сторона: `left` при
        // заданных `left` и `right`.
        let plain = if self.sides[k_start].is_some() {
            self.shift(k_start, own)
        } else {
            self.shift(k_end, own)
        };
        let Some(cb) = cb else {
            return (plain, 0.0, false);
        };
        let (cs, ce) = if x {
            (f(cb.origin.x), f(cb.origin.x) + f(cb.size.width))
        } else {
            (f(cb.origin.y), f(cb.origin.y) + f(cb.size.height))
        };
        let is = self.imcb_edge(k_start, cs, ce - cs);
        let ie = self.imcb_edge(k_end, ce, ce - cs);
        let (explicit, safe) = if x == self.cb_vertical {
            (self.align_self, self.safe_align)
        } else {
            (self.justify_self, self.safe_justify)
        };
        let anchor = if explicit == Some(Align::AnchorCenter) {
            self.lookup(None)
        } else {
            None
        };
        let pos = match anchor {
            Some(a) => {
                let (as_, ae) = if x {
                    (f(a.origin.x), f(a.origin.x) + f(a.size.width))
                } else {
                    (f(a.origin.y), f(a.origin.y) + f(a.size.height))
                };
                let start_bias = !if x {
                    self.cb_flipped.0
                } else {
                    self.cb_flipped.1
                };
                settle_axis(
                    (as_ + ae - mbox) / 2.0,
                    mbox,
                    is,
                    ie,
                    cs,
                    ce,
                    true,
                    start_bias,
                    safe,
                    true,
                )
            }
            None => os - m_s + plain,
        };
        (
            pos + m_s - os,
            (ie - is).max(0.0),
            overflows(pos, mbox, is, ie),
        )
    }

    /// Размещение коробки по плану: клетка `position-area`, а без области
    /// (или без якоря/содержащего блока для неё) — обе оси по вставкам.
    pub(super) fn compute(&self, own: Bounds<Pixels>, window: &Window) -> Placement {
        if let Some(p) = self.area_place(own, window) {
            return p;
        }
        let cb = self.cb_bounds(window);
        let (dx, w, ox) = self.free_axis(true, cb, own);
        let (dy, h, oy) = self.free_axis(false, cb, own);
        Placement {
            dx,
            dy,
            overflow: ox || oy,
            imcb: (w, h),
            cell: (0.0, 0.0),
        }
    }

    /// Размещение в клетке `position-area`; `None` — области нет или
    /// якорь/содержащий блок не найдены (тогда — обычные `anchor()`-вставки).
    fn area_place(&self, own: Bounds<Pixels>, window: &Window) -> Option<Placement> {
        let (tx, ty) = self.area?;
        let a = self.lookup(None)?;
        let cb = self.cb_bounds(window)?;
        let (dx, w, ox, cw) = self.axis_place(true, tx, a, cb, own);
        let (dy, h, oy, ch) = self.axis_place(false, ty, a, cb, own);
        Some(Placement {
            dx,
            dy,
            overflow: ox || oy,
            imcb: (w, h),
            cell: (cw, ch),
        })
    }

    /// Тактика §position-try-fallbacks на готовом плане: `flip-block`/
    /// `flip-inline` — зеркало по оси письма СОДЕРЖАЩЕГО блока («Logical
    /// directions are resolved against the writing mode of the containing
    /// block»): вставки, поля, стороны `anchor()`, дорожки области,
    /// `start`/`end` выравнивания; `flip-start` — транспонирование осей
    /// (диагональ start-start → end-end; для письма с началом не в
    /// левом-верхнем углу — приближение). Составные тактики — по порядку бит.
    pub(super) fn apply_tactics(&mut self, t: u8) {
        if t & FLIP_BLOCK != 0 {
            self.flip_axis(!self.cb_vertical);
        }
        if t & FLIP_INLINE != 0 {
            self.flip_axis(self.cb_vertical);
        }
        if t & FLIP_START != 0 {
            self.transpose();
        }
    }

    fn flip_axis(&mut self, y: bool) {
        let (k1, k2) = if y { (0usize, 2usize) } else { (3, 1) };
        self.sides.swap(k1, k2);
        self.inset.swap(k1, k2);
        self.margin.swap(k1, k2);
        for k in [k1, k2] {
            if let Some(sp) = self.sides[k].as_mut() {
                sp.f.side = mirror_side(sp.f.side, y);
            }
        }
        if let Some((tx, ty)) = self.area.as_mut() {
            let t = if y { ty } else { tx };
            *t = Tracks {
                lo: 3 - t.hi,
                hi: 3 - t.lo,
            };
        }
        // Та же раздача осей, что в `align_for`: `align-self` — блочная ось
        // содержащего блока.
        let slot = if y != self.cb_vertical {
            &mut self.align_self
        } else {
            &mut self.justify_self
        };
        *slot = match *slot {
            Some(Align::Start) => Some(Align::End),
            Some(Align::End) => Some(Align::Start),
            other => other,
        };
    }

    fn transpose(&mut self) {
        self.sides.swap(0, 3);
        self.sides.swap(2, 1);
        self.inset.swap(0, 3);
        self.inset.swap(2, 1);
        self.margin.swap(0, 3);
        self.margin.swap(2, 1);
        for sp in self.sides.iter_mut().flatten() {
            sp.f.side = transpose_side(sp.f.side);
        }
        if let Some((tx, ty)) = self.area.as_mut() {
            std::mem::swap(tx, ty);
        }
        std::mem::swap(&mut self.align_self, &mut self.justify_self);
        std::mem::swap(&mut self.safe_align, &mut self.safe_justify);
    }
}

/// Куда просится край по `anchor()`.
// `FromEdge` names the anchor-relative inset, not a nested `Edge`.
#[allow(clippy::enum_variant_names)]
enum Edge {
    Abs(f32),
    FromEdge(f32),
    None,
}

/// Зажим коробки полей по оси — хвост Blink `ComputeInsets`
/// (`absolute_utils.cc`): при `safe` переполнение прижимает к безопасному
/// краю (начало содержащего блока; у `anchor-center` — только когда центр по
/// якорю выходит за ЭТОТ край, `half_size = c − imcb_start`); иначе при
/// умолчальном переполнении css-align (`shift_in`) коробка сдвигается внутрь
/// IMCB, если в него влезает, иначе внутрь объединения IMCB с содержащим
/// блоком; при нехватке места побеждает край начала
/// (`adjust_end(); adjust_start()`).
#[allow(clippy::too_many_arguments)]
fn settle_axis(
    pos: f32,
    mbox: f32,
    is: f32,
    ie: f32,
    cs: f32,
    ce: f32,
    shift_in: bool,
    start_bias: bool,
    safe: bool,
    anchor_center: bool,
) -> f32 {
    if safe {
        let over = if anchor_center {
            if start_bias {
                pos < is
            } else {
                pos + mbox > ie
            }
        } else {
            mbox > ie - is
        };
        return if !over {
            pos
        } else if start_bias {
            is
        } else {
            ie - mbox
        };
    }
    if !shift_in {
        return pos;
    }
    let (lo, hi) = if mbox <= ie - is {
        (is, ie - mbox)
    } else {
        (is.min(cs), ie.max(ce) - mbox)
    };
    if start_bias {
        pos.min(hi).max(lo)
    } else {
        pos.max(lo).min(hi)
    }
}

/// Переполняет ли коробка полей `[pos, pos+mbox]` IMCB `[is, ie]` (Blink
/// `CalculateNonOverflowingRangeInOneAxis`: любой край за краем IMCB).
fn overflows(pos: f32, mbox: f32, is: f32, ie: f32) -> bool {
    pos < is - 0.01 || pos + mbox > ie + 0.01
}

/// Зеркало `<anchor-side>` по оси тактики (§fallback, execute a try-tactic):
/// физические стороны оси, `start`↔`end`, `self-start`↔`self-end`,
/// `<pct>` → `100% − <pct>`; `inside`/`outside` относительны и не меняются.
fn mirror_side(s: AnchorSide, y: bool) -> AnchorSide {
    match s {
        AnchorSide::Top if y => AnchorSide::Bottom,
        AnchorSide::Bottom if y => AnchorSide::Top,
        AnchorSide::Left if !y => AnchorSide::Right,
        AnchorSide::Right if !y => AnchorSide::Left,
        AnchorSide::Start => AnchorSide::End,
        AnchorSide::End => AnchorSide::Start,
        AnchorSide::SelfStart => AnchorSide::SelfEnd,
        AnchorSide::SelfEnd => AnchorSide::SelfStart,
        AnchorSide::Pct(p) => AnchorSide::Pct(1.0 - p),
        other => other,
    }
}

/// `flip-start`: физические стороны меняются осями, логические остаются.
fn transpose_side(s: AnchorSide) -> AnchorSide {
    match s {
        AnchorSide::Top => AnchorSide::Left,
        AnchorSide::Left => AnchorSide::Top,
        AnchorSide::Bottom => AnchorSide::Right,
        AnchorSide::Right => AnchorSide::Bottom,
        other => other,
    }
}
