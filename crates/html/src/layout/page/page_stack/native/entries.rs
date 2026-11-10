//! Точные размещения native OOF по листам и допуск их paint-копий.
use super::*;
use crate::layout::fragment::axis_slice::{self as geometry, snapped};
use crate::layout::fragment::token::resume::oof::FixedPolicy;
use crate::layout::positioned::containing_block::metrics::Snapshot;
use crate::layout::positioned::containing_block::metrics::fragmentation::paint::{
    Admission, Placement,
};
use std::rc::Rc;

impl PageStack {
    pub(super) fn native_entries<const QUERY: bool>(
        &self,
        bounds: Bounds<Pixels>,
        normal: &[Frag],
        window: &Window,
    ) -> Result<Vec<(usize, Rc<Snapshot>, Admission)>, PaintRecordError> {
        let plan = self.oof.plan.borrow();
        if plan.failure.is_some() {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let sources = self.oof.sources.borrow();
        let snapshots = self.oof.snapshots.borrow();
        let repeats = self.oof.repeat_snapshots.borrow();
        let viewport = window.viewport_size();
        let spill = f32::from(viewport.width.max(viewport.height).max(bounds.size.width));
        let scale = if QUERY { 0. } else { window.scale_factor() };
        let mut entries: Vec<(usize, Rc<Snapshot>, Admission)> = Vec::new();
        for f in plan.iter() {
            let (owner, source) = sources
                .get(f.kid)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let snapshot = if source.policy == FixedPolicy::RepeatPages {
                repeats
                    .iter()
                    .find(|(page, o, _)| *page == f.col && o == owner)
                    .map(|(_, _, s)| s)
            } else {
                snapshots.iter().find(|(o, _)| o == owner).map(|(_, s)| s)
            }
            .ok_or(PaintRecordError::InvalidGeometry)?;
            let flow = snapshot
                .flows
                .iter()
                .find(|s| s.id == source.id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            if !flow.native {
                continue;
            }
            let rect = flow.metrics.normal_rect();
            // The source paints the box where its layout put it; the normal
            // rect above holds the resolved static position the plan used.
            let laid = snapshot.normal.get(&flow.id).copied().unwrap_or(rect);
            let inline = geometry::inline(self.axis, rect);
            let from = geometry::slice(self.axis, laid, 0., inline, f.from, f.h).origin;
            let offset = if self.axis.is_vertical() {
                rect.origin.y
            } else {
                rect.origin.x
            };
            let page = self.page_area(bounds, f.col);
            let to = geometry::slice(self.axis, page, offset.into(), inline, f.y, f.h).origin;
            let exact = to;
            let to = if QUERY {
                to
            } else {
                from + snapped(to - from, scale)
            };
            // Unbroken edges keep their ink overflow; a break edge cuts it.
            let siblings = || plan.iter().filter(|g| g.kid == f.kid);
            let first = !siblings().any(|g| (g.col, g.from) < (f.col, f.from));
            let last = !siblings().any(|g| (g.col, g.from) > (f.col, f.from));
            let (head, tail) = flow.kid.clone_dec.unwrap_or_default();
            let up = if first { spill } else { -head };
            let down = if last { spill } else { -tail };
            let clip = geometry::slice(
                self.axis,
                laid,
                -spill,
                inline + spill * 2.,
                f.from - up,
                (f.h + up + down).max(0.),
            );
            // The group's copy on this sheet carries the root's own slot.
            // The box's own decoration spans the whole fragment: a broken box
            // fills the rest of its fragmentainer (css-break-3 §5.4).
            let extent = if last {
                f.h
            } else {
                (self.area_logical(&self.geom(f.col)).1 - f.y).max(f.h)
            };
            let decoration = geometry::slice(
                self.axis,
                laid,
                -spill,
                inline + spill * 2.,
                f.from - if first { spill } else { 0. },
                extent + if first { spill } else { 0. } + if last { spill } else { 0. },
            );
            // The plan placed normal geometry; relative positioning of the box and its
            // ancestors is a paint displacement on top (CSS 2.1 §9.4.3).
            let reference = geometry::slice(self.axis, laid, 0., inline, f.from, f.h);
            let (clip, decoration) = if QUERY {
                (reference, reference)
            } else {
                (clip, decoration)
            };
            let shift = snapshot.relative.get(&flow.id).copied().unwrap_or_default();
            let copied = normal.iter().any(|g| g.kid == *owner && g.col == f.col);
            let index = match entries.iter().position(|e| Rc::ptr_eq(&e.1, snapshot)) {
                Some(index) => index,
                None => {
                    entries.push((
                        *owner,
                        snapshot.clone(),
                        Admission::new(self.axis.is_vertical()).snapping(scale),
                    ));
                    entries.len() - 1
                }
            };
            entries[index].2.insert(Placement {
                family: flow.id,
                root: flow.root,
                segment: PaintSegmentId(f.col as u64),
                source: from,
                destination: to + shift,
                exact: exact + shift,
                clip,
                decoration,
                copied,
                transform: flow.transform,
                reference,
                start: geometry::slice(self.axis, page, offset.into(), inline, 0., 0.).origin
                    - (to + shift - from),
            })?;
        }
        Ok(entries)
    }
}
