//! Нативная краска обычного потока ребёнка стопки: главная копия раскладывается и
//! prepaint-ится один раз, её краска пишется одной записью по сегментам кусков плана
//! (`record_paint_segments`, сегмент — номер копии куска) и воспроизводится в каждом куске
//! под его срезом; hitboxes ставятся по тем же размещениям и срезам (как OOF-продолжения
//! P08). Запасные копии такого ребёнка не раскладываются и не рисуются. Геометрия куска
//! (место копии и срез `slice`) — общая для этого пути и для копий.

use crate::layout::fragment::effect_fragment::pieces::{self, Piece, Pieces};
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{
    App, Bounds, PaintRecordError, PaintSegment, PaintSegmentId, PaintSegmentRecords,
    PaintSegmentSlice, Pixels, Window, point, px,
};
use std::rc::Rc;
mod play;

/// Размещения кусков одного ребёнка и их срезы в координатах главной копии.
pub(super) struct Routes {
    routes: Rc<[PaintSegment]>,
    slices: Rc<[PaintSegmentSlice]>,
    extent: Bounds<Pixels>,
    nodes: Box<super::admission::Nodes>,
    watched: Option<Rc<crate::layout::fragment::paginate::watch::Watched>>,
    /// Куски для художников, читающих границы фрагмента (`StackChild::effects`).
    pieces: Option<Rc<[Piece]>>,
}

/// Запись главной копии и расширенные маски её кусков.
pub(super) type Replayed = (
    PaintSegmentRecords,
    std::collections::HashMap<usize, Bounds<Pixels>>,
);

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
        let mut pieces = Vec::new();
        let mut effects = Vec::new();
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
            pieces.push((f, placement, source_clip));
            let fragment = Bounds::new(at, gpui::size(px(col_w), px(f.h)));
            effects.push(Piece {
                placement,
                fragment: Bounds::new(fragment.origin - delta, fragment.size),
                clip: Bounds::new(clip.origin - delta, clip.size),
                slice: source_clip,
                tail: px(self.gap_tail(f, plan_all)),
                admitted: false,
            });
            // Возвращённые записи художников фрагмента стоят уже на месте куска.
            if self.children[kid].effects {
                let reach = px(f32::from(spill));
                let placed = Bounds::from_corners(
                    point(mask.left(), mask.top() - reach),
                    point(mask.right(), mask.bottom() + reach),
                );
                extent = extent.map(|e| e.union(&placed));
            }
            routes.push(placement);
            slices.push(PaintSegmentSlice {
                placement,
                source_clip,
            });
        }
        let nodes = self.node_admission(kid, &pieces, spill);
        let fx = self.children[kid].effects;
        routes.extend(nodes.routes.iter().copied());
        let extent = nodes
            .slices
            .values()
            .flat_map(|s| s.iter())
            .fold(extent?, |e, s| e.union(&s.source_clip));
        Some(Routes {
            routes: routes.into(),
            slices: slices.into(),
            extent,
            nodes: Box::new(nodes),
            watched: self.children[kid].watched.clone(),
            pieces: fx.then(|| effects.into()),
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
        routes.admit(true);
        let done = window.with_prepaint_segments(
            routes.extent,
            routes.routes.clone(),
            cx,
            |window, cx| window.with_paint_segment_slices(slices, |window| prepaint(window, cx)),
        );
        routes.admit(false);
        done?
    }

    /// Запись краски главной копии по сегментам кусков.
    pub(super) fn record_replayed(
        routes: &Routes,
        (el, owner): (
            &mut gpui::AnyElement,
            &crate::layout::cx::deferred::Deferred,
        ),
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Replayed, PaintRecordError> {
        let slices = routes.slices.clone();
        let fx = (routes.pieces.clone()).map(|p| Rc::new(Pieces::new(p, routes.watched.clone())));
        routes.admit(true);
        let recorded = pieces::with(owner, fx.clone(), || {
            window.record_paint_segments(routes.extent, routes.routes.clone(), cx, |window, cx| {
                window.with_paint_segment_slices(slices, |window| el.paint(window, cx))
            })
        });
        routes.admit(false);
        let (painted, records) = recorded?;
        painted?;
        let mut masks = routes.nodes.masks.clone();
        for (segment, b) in fx.iter().flat_map(|p| p.extra.take()) {
            let m = masks.entry(segment as usize).or_insert(b);
            *m = m.union(&b);
        }
        Ok((records, masks))
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
            let el = (&mut self.children[f.kid].el, &*self.paint);
            slot.insert(routes.map(|r| Self::record_replayed(&r, el, window, cx).ok()));
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
        let mut mask = crate::layout::fragment::fragment_mask::snap(clip, scale);
        if let Some(extra) = masks.get(&f.copy) {
            mask.bounds = mask.bounds.union(extra);
        }
        window.with_content_mask(Some(mask), |window| {
            let _ = Self::paint_replayed(records, f.copy, window);
        });
        true
    }
}

impl Routes {
    /// Допуск узлов главной копии на время её prepaint или записи.
    fn admit(&self, on: bool) {
        if let Some(w) = &self.watched
            && !self.nodes.slices.is_empty()
        {
            w.set_admission(if on {
                self.nodes.slices.clone()
            } else {
                Default::default()
            });
        }
    }
}
