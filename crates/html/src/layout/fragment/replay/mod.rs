//! Запись главной копии разрезанного ребёнка стопки фрагментации и её воспроизведение по
//! кускам: главная копия раскладывается и prepaint-ится один раз, её краска пишется одной
//! записью по сегментам кусков плана (`record_paint_segments`, сегмент — номер копии куска)
//! и играется в каждом куске под его срезом; hitboxes ставятся по тем же размещениям и
//! срезам (как OOF-продолжения P08). Место и срез куска даёт стопка (`PieceGeometry`):
//! колонки многоколоночника и листы печати.

use crate::layout::cx::deferred::Deferred;
use crate::layout::fragment::effect_fragment::pieces::{self, Piece, Pieces};
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::replay_pieces::ReplayPiece;
use crate::layout::fragment::types::Frag;
use gpui::{
    App, Bounds, PaintRecordError, PaintSegment, PaintSegmentId, PaintSegmentRecords,
    PaintSegmentSlice, Pixels, Point, Window, point, px, size,
};
use std::rc::Rc;
mod admission;
mod play;
mod scopes;
mod slots;
pub(crate) use crate::layout::fragment::replay_pieces::LATE;
pub(crate) use play::{late_segments, paint_late, paint_segment};

/// Геометрия кусков ребёнка в координатах записи, которую даёт его стопка.
pub(crate) trait PieceGeometry {
    /// Начало куска `f` до подъёма копии на `from`.
    fn piece_at(&self, f: &Frag) -> Point<Pixels>;
    /// Срез куска, стоящего в `at` (`slice`, css-break-3 §4), и маска, под которой его
    /// рисует копия (срез, снятый на пиксели устройства).
    fn piece_clip(&self, f: &Frag, at: Point<Pixels>) -> (Bounds<Pixels>, Bounds<Pixels>);
    /// Ширина фрагментаинера куска по строчной оси.
    fn fragment_width(&self, f: &Frag) -> f32;
    /// Область краски слотов куска: маска или полоса фрагментаинера.
    fn slot_area(&self, f: &Frag, clip: Bounds<Pixels>, mask: Bounds<Pixels>) -> Bounds<Pixels>;
    /// Запись, перенесённая на `delta`, равна копии на месте куска: копия снимает края
    /// своих декораций на сетку устройства на своём месте, запись переносит уже снятые.
    fn exact_shift(&self, delta: Point<Pixels>) -> bool;
    /// Свой срез содержимого куска в координатах источника (`clone`); `None` — маска.
    fn content_slice(
        &self,
        _f: &Frag,
        _source: Point<Pixels>,
        _across: (Pixels, Pixels),
    ) -> Option<Bounds<Pixels>> {
        None
    }
    /// Пустой хвост фрагментаинера за разрезом куска (`gap_fragment::Scope::end`).
    fn gap_tail(&self, _f: &Frag) -> f32 {
        0.0
    }
}

/// Ребёнок, чья главная копия пишется: его раскладка для допуска узлов, художники
/// фрагмента и корень clone. `monoliths` — монолит, переполнивший кусок своего начала,
/// рисуется только в нём (колонки); листы печати продолжают его переполнение на следующих
/// листах (`PageStack::spill_overflow`), как копии.
pub(crate) struct Source<'a> {
    pub monoliths: bool,
    pub regrow: Option<&'a crate::layout::fragment::push::Regrow>,
    pub watched: Option<&'a Rc<Watched>>,
    pub effects: bool,
    pub clone_root: Option<u64>,
}

/// Размещения кусков одного ребёнка и их срезы в координатах главной копии.
pub(crate) struct Routes {
    routes: Rc<[PaintSegment]>,
    slices: Rc<[PaintSegmentSlice]>,
    extent: Bounds<Pixels>,
    nodes: Box<admission::Nodes>,
    watched: Option<Rc<Watched>>,
    /// Куски для художников, читающих границы фрагмента.
    pieces: Option<Rc<[Piece]>>,
    /// Куски для слотов внепоточных корней (`continuation::replayed`).
    slots: Rc<[ReplayPiece]>,
    /// Фрагмент каждого куска по блочной оси в координатах источника (`line_admission`).
    frags: Vec<(f32, f32)>,
    spill: Pixels,
    /// Корень clone: его собственная краска в записи скрыта (`clone_hidden`).
    clone_root: Option<u64>,
}

/// Запись главной копии и расширенные маски её кусков (в координатах записи).
pub(crate) type Replayed = (
    PaintSegmentRecords,
    std::collections::HashMap<usize, Bounds<Pixels>>,
);

impl Routes {
    /// Размещения всех кусков ребёнка `kid`: источник — место главной копии, срез —
    /// снятая маска куска, перенесённая в координаты источника. `None` — перенос не на
    /// целые пиксели устройства: куски рисуют свои копии.
    pub(crate) fn new(
        kid: usize,
        plan_all: &[Frag],
        g: &impl PieceGeometry,
        src: Source,
        spill: Pixels,
    ) -> Option<Self> {
        let main = plan_all.iter().find(|f| f.kid == kid && f.copy == 0)?;
        let source = g.piece_at(main) - point(px(0.0), px(main.from));
        let (mut routes, mut slices, mut pieces, mut effects) = (vec![], vec![], vec![], vec![]);
        let (mut slots, mut frags) = (vec![], vec![]);
        let mut extent: Option<Bounds<Pixels>> = None;
        for f in plan_all.iter().filter(|f| f.kid == kid) {
            let at = g.piece_at(f);
            let destination = at - point(px(0.0), px(f.from));
            let delta = destination - source;
            if !g.exact_shift(delta) {
                return None;
            }
            let (clip, mask) = g.piece_clip(f, at);
            let placement = PaintSegment {
                id: PaintSegmentId(f.copy as u64),
                ordinal: 0,
                source,
                destination,
            };
            slots.push(slots::slot_piece(f, placement, g.slot_area(f, clip, mask)));
            let source_clip = Bounds::new(mask.origin - delta, mask.size);
            let across = (source_clip.left(), source_clip.right());
            let source_clip = g.content_slice(f, source, across).unwrap_or(source_clip);
            extent = Some(extent.map_or(source_clip, |e| e.union(&source_clip)));
            pieces.push((f, placement, source_clip));
            let fragment = Bounds::new(at, size(px(g.fragment_width(f)), px(f.h)));
            let top = f32::from(fragment.origin.y - delta.y);
            frags.push((top, top + f.h));
            effects.push(Piece {
                placement,
                fragment: Bounds::new(fragment.origin - delta, fragment.size),
                clip: Bounds::new(clip.origin - delta, clip.size),
                slice: source_clip,
                tail: px(g.gap_tail(f)),
                admitted: false,
                end_clipped: false,
            });
            // Возвращённые записи художников фрагмента стоят уже на месте куска.
            if src.effects {
                let placed = Bounds::from_corners(
                    point(mask.left(), mask.top() - spill),
                    point(mask.right(), mask.bottom() + spill),
                );
                extent = extent.map(|e| e.union(&placed));
            }
            routes.push(placement);
            slices.push(PaintSegmentSlice {
                placement,
                source_clip,
            });
        }
        let extent = slots::slot_routes(&slots, &mut routes, extent);
        let nodes = admission::nodes(&src, &pieces, spill);
        routes.extend(nodes.routes.iter().copied());
        let extent = nodes
            .slices
            .values()
            .flat_map(|s| s.iter())
            .fold(extent?, |e, s| e.union(&s.source_clip));
        Some(Self {
            routes: routes.into(),
            slices: slices.into(),
            extent,
            nodes: Box::new(nodes),
            watched: src.watched.cloned(),
            pieces: src.effects.then(|| effects.into()),
            slots: slots.into(),
            frags,
            spill,
            clone_root: src.clone_root,
        })
    }

    /// Запись краски главной копии `el` по сегментам кусков; `owner` — хозяин кусков
    /// художников фрагмента, `records` — нативные записи стопки для слотов.
    pub(crate) fn record(
        &self,
        (el, owner, records): (
            &mut gpui::AnyElement,
            Option<&Deferred>,
            Option<crate::hooks::BareRecords>,
        ),
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Replayed, PaintRecordError> {
        let fx = (self.pieces.clone()).map(|p| Rc::new(Pieces::new(p, self.watched.clone())));
        let natives = records.as_ref().is_some_and(|r| !r.is_empty());
        self.admit(true);
        let recorded = match owner {
            Some(owner) => pieces::with(owner, fx.clone(), || {
                self.record_scopes(records, window, |w| self.paint_into(el, w, cx))
            }),
            None => self.record_scopes(records, window, |w| self.paint_into(el, w, cx)),
        };
        self.admit(false);
        let ((painted, records), lines) = (recorded.0?, recorded.1);
        painted?;
        let masks = self.masks(natives, fx.iter().flat_map(|p| p.extra.take()), &lines);
        Ok((records, masks))
    }

    #[allow(clippy::type_complexity)]
    fn paint_into(
        &self,
        el: &mut gpui::AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(Result<(), PaintRecordError>, PaintSegmentRecords), PaintRecordError> {
        let slices = self.slices.clone();
        window.record_paint_segments(self.extent, self.routes.clone(), cx, |window, cx| {
            window.with_paint_segment_slices(slices, |window| el.paint(window, cx))
        })
    }

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
