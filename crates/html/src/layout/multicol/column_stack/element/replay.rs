//! Нативная краска обычного потока ребёнка колонок: запись главной копии
//! (`layout::fragment::replay`) с геометрией куска колонки — место копии и срез `slice`,
//! общие для этого пути и для копий. Запасные копии такого ребёнка не раскладываются и
//! не рисуются.

use crate::layout::fragment::fragment_mask;
use crate::layout::fragment::replay::{self, PieceGeometry, Replayed, Routes, Source};
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, Pixels, Point, Window};
mod clone;

/// Геометрия кусков колонок одного кадра стопки.
struct Columns<'a> {
    stack: &'a ColumnStack,
    plan: &'a [Frag],
    bounds: Bounds<Pixels>,
    col_w: f32,
    step: f32,
    spill: Pixels,
    scale: f32,
}

impl PieceGeometry for Columns<'_> {
    fn piece_at(&self, f: &Frag) -> Point<Pixels> {
        self.stack.piece_at(f, self.bounds, self.step)
    }

    fn piece_clip(&self, f: &Frag, at: Point<Pixels>) -> (Bounds<Pixels>, Bounds<Pixels>) {
        let geometry = (self.col_w, self.spill);
        let (clip, _) = self.stack.piece_clip(f, at, geometry, self.plan, false);
        (clip, fragment_mask::snap(clip, self.scale).bounds)
    }

    fn fragment_width(&self, _f: &Frag) -> f32 {
        self.col_w
    }

    fn slot_area(&self, f: &Frag, clip: Bounds<Pixels>, mask: Bounds<Pixels>) -> Bounds<Pixels> {
        (self
            .stack
            .fragmentainer_band(f.col, self.bounds, self.spill))
        .map_or(mask, |b| {
            fragment_mask::snap(b.union(&clip), self.scale).bounds
        })
    }

    fn exact_shift(&self, delta: Point<Pixels>) -> bool {
        crate::layout::fragment::axis_slice::snapped(delta, self.scale) == delta
    }

    fn content_slice(
        &self,
        f: &Frag,
        source: Point<Pixels>,
        across: (Pixels, Pixels),
    ) -> Option<Bounds<Pixels>> {
        self.stack.clone_slice(f, self.plan, source, across)
    }

    fn gap_tail(&self, f: &Frag) -> f32 {
        self.stack.gap_tail(f, self.plan)
    }
}

impl ColumnStack {
    /// Ребёнок, чьи куски рисует запись главной копии: без собственной геометрии копий
    /// (`clone`) и без исключений поддерева (`StackChild::replay`). Полосы повтора таблицы и
    /// хвост её фрагмента рисуются при этом своими копиями и заливкой (`paint_frags`): поток
    /// продолжения стоит под шапкой уже в плане (`f.y`).
    /// Решение одно для prepaint и paint: от контекста краски оно не зависит.
    pub(super) fn replays(&self, kid: usize, plan: &[Frag]) -> bool {
        let c = &self.children[kid];
        self.replay
            && c.replay
            && (c.clone_dec.is_none() || c.clone_deco.is_some())
            && plan.iter().any(|f| f.kid == kid && f.copy == 0)
            && plan.iter().filter(|f| f.kid == kid).count() > 1
    }

    /// Размещения всех кусков ребёнка `kid` в колонках.
    pub(super) fn replay_routes(
        &self,
        kid: usize,
        plan: &[Frag],
        bounds: Bounds<Pixels>,
        (col_w, step, spill): (f32, f32, Pixels),
        scale: f32,
    ) -> Option<Routes> {
        let geometry = Columns {
            stack: self,
            plan,
            bounds,
            col_w,
            step,
            spill,
            scale,
        };
        let c = &self.children[kid];
        let source = Source {
            monoliths: true,
            regrow: c.regrow.as_ref(),
            watched: c.watched.as_ref(),
            effects: c.effects,
            clone_root: c.clone_deco.as_ref().map(|d| d.node),
        };
        Routes::new(kid, plan, &geometry, source, spill)
    }

    /// Кусок `f` ребёнка, рисуемого записью главной копии: запись — при первом его
    /// куске, затем каждый кусок воспроизводит свой сегмент под своей маской. Ложь —
    /// размещений нет, кусок рисует своя копия (тот же выбор, что в prepaint).
    pub(super) fn paint_piece_replayed(
        &mut self,
        f: &Frag,
        plan_all: &[Frag],
        (bounds, col_w, step): (Bounds<Pixels>, f32, f32),
        replayed: &mut std::collections::HashMap<usize, Option<Option<Replayed>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let spill = Self::spill(bounds, window);
        let scale = window.scale_factor();
        if let std::collections::hash_map::Entry::Vacant(slot) = replayed.entry(f.kid) {
            let routes = self.replay_routes(f.kid, plan_all, bounds, (col_w, step, spill), scale);
            let records = self.copy_records();
            let el = (&mut self.children[f.kid].el, Some(&*self.paint), records);
            slot.insert(routes.map(|r| r.record(el, window, cx).ok()));
        }
        // Без размещений ребёнок рисуется копиями; неудавшаяся запись — ничем.
        let (records, masks) = match replayed.get(&f.kid) {
            Some(Some(Some((records, masks)))) => (records, masks),
            Some(Some(None)) => return true,
            _ => return false,
        };
        let at = self.piece_at(f, bounds, step);
        let (clip, _) = self.piece_clip(f, at, (col_w, spill), plan_all, false);
        let parent = window.content_mask();
        if let Some(w) = &self.children[f.kid].watched {
            w.set_paint_outer(Some(parent));
        }
        let mut mask = fragment_mask::snap(clip, scale);
        if let Some(extra) = masks.get(&f.copy) {
            mask.bounds = mask.bounds.union(extra);
        }
        // Фрагмент clone самодостаточен: украшения — копией без содержимого, маски нет.
        let decos = self.children[f.kid].clone_deco.as_mut();
        if let Some(deco) = decos.and_then(|d| d.decos.iter_mut().nth(f.copy)) {
            deco.paint(window, cx);
            mask = parent;
        }
        window.with_content_mask(Some(mask), |window| {
            let _ = replay::paint_segment(records, f.copy, window);
        });
        true
    }
}
