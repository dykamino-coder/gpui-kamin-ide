//! Нативная краска обычного потока ребёнка стопки: главная копия раскладывается и
//! prepaint-ится один раз, её краска пишется одной записью по сегментам кусков плана
//! (`record_paint_segments`, сегмент — номер копии куска) и воспроизводится в каждом куске
//! под его срезом; hitboxes ставятся по тем же размещениям и срезам (как OOF-продолжения
//! P08). Запасные копии такого ребёнка не раскладываются и не рисуются. Геометрия куска
//! (место копии и срез `slice`) — общая для этого пути и для копий.

use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintRecordError, PaintSegment, PaintSegmentId,
    PaintSegmentRecords, PaintSegmentSlice, Pixels, Point, Window, point, px, size,
};
use std::rc::Rc;

/// Размещения кусков одного ребёнка и их срезы в координатах главной копии.
pub(super) struct Routes {
    routes: Rc<[PaintSegment]>,
    slices: Rc<[PaintSegmentSlice]>,
    extent: Bounds<Pixels>,
}

impl ColumnStack {
    /// Ребёнок, чьи куски рисует запись главной копии: без собственной геометрии копий
    /// (`clone`, полосы повтора, хвост таблицы), без слотов OOF и статических мест по
    /// сегментам и без художников, читающих границы фрагмента (`StackChild::replay`).
    /// Решение одно для prepaint и paint: от контекста краски оно не зависит.
    pub(super) fn replays(&self, kid: usize, plan: &[Frag]) -> bool {
        let c = &self.children[kid];
        self.replay
            && c.replay
            && c.clone_dec.is_none()
            && c.repeat.is_none()
            && c.slack.is_none()
            && !c.nested_cols
            && self.copy_records().is_none()
            && self.static_places.is_none()
            && plan.iter().any(|f| f.kid == kid && f.copy == 0)
            && plan.iter().filter(|f| f.kid == kid).count() > 1
    }

    /// Начало куска `f` в колонке (до подъёма копии на `from`).
    pub(super) fn piece_at(&self, f: &Frag, bounds: Bounds<Pixels>, step: f32) -> Point<Pixels> {
        let (c, ry) = self.place(f.col);
        let k = &self.children[f.kid];
        point(
            bounds.origin.x + px(c as f32 * step + k.rel.0 + k.par.dx),
            bounds.origin.y + px(ry + f.y + k.rel.1),
        )
    }

    /// Вылет среза вбок: на ширину окна, но не уже стопки (css-multicol-1 §8.1).
    pub(super) fn spill(bounds: Bounds<Pixels>, window: &Window) -> Pixels {
        window.viewport_size().width.max(bounds.size.width)
    }

    /// Срез куска `f`, стоящего в `at` (`slice`, css-break-3 §4): режутся только
    /// внутренние края фрагментов; чернила над первым и под последним видны (§5.4), под
    /// последним — до низа колонки. `flat` — таблица и вложенная фрагментация задают свои
    /// границы. Второе — продолжается ли ребёнок следующим куском.
    pub(super) fn piece_clip(
        &self,
        f: &Frag,
        at: Point<Pixels>,
        (col_w, spill): (f32, Pixels),
        plan_all: &[Frag],
        flat: bool,
    ) -> (Bounds<Pixels>, bool) {
        let line = if self.wraps() { f.col / self.count } else { 0 };
        let line_h = self.lines_plan.borrow().get(line).map_or(0.0, |l| l.1);
        let continued = plan_all
            .iter()
            .any(|g| g.kid == f.kid && g.copy == f.copy + 1);
        let over = self.children[f.kid].over;
        let (up, down) = if flat {
            (0.0, 0.0)
        } else {
            (
                if f.copy == 0 && f.from <= COORD_EPS {
                    f32::from(spill)
                } else {
                    0.0
                },
                if continued || f.from + f.h < over - COORD_EPS {
                    0.0
                } else {
                    (line_h - f.y - f.h).max(0.0)
                },
            )
        };
        let clip = Bounds {
            origin: point(at.x - spill, at.y - px(up)),
            size: size(px(col_w) + spill + spill, px(f.h + up + down)),
        };
        (clip, continued)
    }

    /// Размещения всех кусков ребёнка `kid`: источник — место главной копии, срез —
    /// снятая на пиксели маска куска, перенесённая в координаты источника.
    pub(super) fn replay_routes(
        &self,
        kid: usize,
        plan_all: &[Frag],
        bounds: Bounds<Pixels>,
        (col_w, step, spill): (f32, f32, Pixels),
        scale: f32,
    ) -> Option<Routes> {
        let main = plan_all.iter().find(|f| f.kid == kid && f.copy == 0)?;
        let source = self.piece_at(main, bounds, step) - point(px(0.0), px(main.from));
        let mut routes = Vec::new();
        let mut slices = Vec::new();
        let mut extent: Option<Bounds<Pixels>> = None;
        for f in plan_all.iter().filter(|f| f.kid == kid) {
            let at = self.piece_at(f, bounds, step);
            // Копия снимает края своих декораций на сетку устройства на своём месте;
            // запись переносит уже снятые края. Равны они только при переносе на целые
            // пиксели устройства — иначе куски рисуют свои копии.
            let destination = at - point(px(0.0), px(f.from));
            let delta = destination - source;
            if crate::layout::fragment::axis_slice::snapped(delta, scale) != delta {
                return None;
            }
            let (clip, _) = self.piece_clip(f, at, (col_w, spill), plan_all, false);
            let mask = crate::layout::fragment::fragment_mask::snap(clip, scale).bounds;
            let placement = PaintSegment {
                id: PaintSegmentId(f.copy as u64),
                ordinal: 0,
                source,
                destination,
            };
            let source_clip = Bounds::new(mask.origin - (destination - source), mask.size);
            extent = Some(extent.map_or(source_clip, |e| e.union(&source_clip)));
            routes.push(placement);
            slices.push(PaintSegmentSlice {
                placement,
                source_clip,
            });
        }
        Some(Routes {
            routes: routes.into(),
            slices: slices.into(),
            extent: extent?,
        })
    }

    /// Prepaint главной копии: её hitboxes стоят в каждом куске под его срезом.
    pub(super) fn prepaint_replayed(
        routes: &Routes,
        window: &mut Window,
        cx: &mut App,
        prepaint: impl FnOnce(&mut Window, &mut App),
    ) -> Result<(), PaintRecordError> {
        let slices = routes.slices.clone();
        window.with_prepaint_segments(routes.extent, routes.routes.clone(), cx, |window, cx| {
            window.with_paint_segment_slices(slices, |window| prepaint(window, cx))
        })?
    }

    /// Запись краски главной копии по сегментам кусков.
    pub(super) fn record_replayed(
        routes: &Routes,
        el: &mut gpui::AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<PaintSegmentRecords, PaintRecordError> {
        let slices = routes.slices.clone();
        let (painted, records) = window.record_paint_segments(
            routes.extent,
            routes.routes.clone(),
            cx,
            |window, cx| window.with_paint_segment_slices(slices, |window| el.paint(window, cx)),
        )?;
        painted?;
        Ok(records)
    }

    /// Кусок `copy` из записи главной копии.
    pub(super) fn paint_replayed(
        records: &PaintSegmentRecords,
        copy: usize,
        window: &mut Window,
    ) -> Result<(), PaintRecordError> {
        let Some(record) = records.get(PaintSegmentId(copy as u64)) else {
            return Ok(());
        };
        window.paint_plan(
            &PaintPlan::record(record.clone()),
            PaintPlacement {
                source: point(px(0.), px(0.)),
                destination: point(px(0.), px(0.)),
                clip: window.content_mask().bounds,
            },
        )?;
        Ok(())
    }

    /// Кусок `f` ребёнка, рисуемого записью главной копии: запись — при первом его
    /// куске, затем каждый кусок воспроизводит свой сегмент под своей маской. Ложь —
    /// размещений нет, кусок рисует своя копия (тот же выбор, что в prepaint).
    pub(super) fn paint_piece_replayed(
        &mut self,
        f: &Frag,
        plan_all: &[Frag],
        (bounds, col_w, step): (Bounds<Pixels>, f32, f32),
        replayed: &mut std::collections::HashMap<usize, Option<Option<PaintSegmentRecords>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let spill = Self::spill(bounds, window);
        let scale = window.scale_factor();
        if let std::collections::hash_map::Entry::Vacant(slot) = replayed.entry(f.kid) {
            let routes = self.replay_routes(f.kid, plan_all, bounds, (col_w, step, spill), scale);
            let el = &mut self.children[f.kid].el;
            slot.insert(routes.map(|r| Self::record_replayed(&r, el, window, cx).ok()));
        }
        // Без размещений ребёнок рисуется копиями; неудавшаяся запись — ничем.
        let records = match replayed.get(&f.kid) {
            Some(Some(Some(records))) => records,
            Some(Some(None)) => return true,
            _ => return false,
        };
        let at = self.piece_at(f, bounds, step);
        let (clip, _) = self.piece_clip(f, at, (col_w, spill), plan_all, false);
        let parent = window.content_mask();
        if let Some(w) = &self.children[f.kid].watched {
            w.set_paint_outer(Some(parent));
        }
        let mask = crate::layout::fragment::fragment_mask::snap(clip, scale);
        window.with_content_mask(Some(mask), |window| {
            let _ = Self::paint_replayed(records, f.copy, window);
        });
        true
    }
}
