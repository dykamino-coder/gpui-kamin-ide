//! Независимые OOF-продолжения листов: единственный native paint исходной группы
//! маршрутизируется по сегментам-листам (ключ — лист и порядковый номер размещения);
//! лист воспроизводится под своей матрицей листа, она же — матрица hit testing.

use super::PageStack;
use crate::layout::fragment::axis_slice as geometry;
use crate::layout::fragment::token::resume::oof::FixedPolicy;
use crate::layout::positioned::containing_block::metrics::Snapshot;
use crate::layout::positioned::containing_block::metrics::fragmentation::paint::Admission;
use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintRecord, PaintRecordError, PaintSegment,
    PaintSegmentId, PaintSegmentSlice, Pixels, TransformationMatrix, Window, point, px, size,
};
use std::{collections::HashMap, rc::Rc};

/// One native source (a group's snapshot, or a fixed repeat's page snapshot).
struct Entry {
    owner: usize,
    snapshot: Rc<Snapshot>,
    routes: Vec<PaintSegment>,
    ordinals: HashMap<usize, u32>,
    admission: Admission,
}

impl Entry {
    fn new(owner: usize, snapshot: Rc<Snapshot>) -> Self {
        Self {
            owner,
            snapshot,
            routes: Vec::new(),
            ordinals: HashMap::new(),
            admission: Admission::new(),
        }
    }

    fn place(
        &mut self,
        family: u64,
        page: usize,
        source: gpui::Point<Pixels>,
        destination: gpui::Point<Pixels>,
        clip: Bounds<Pixels>,
    ) -> Result<(), PaintRecordError> {
        let ordinal = self.ordinals.entry(page).or_default();
        let placement = PaintSegment {
            id: PaintSegmentId(page as u64),
            ordinal: *ordinal,
            source,
            destination,
        };
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(PaintRecordError::InvalidGeometry)?;
        self.routes.push(placement);
        self.admission.insert(
            family,
            PaintSegmentSlice {
                placement,
                source_clip: clip,
            },
        )
    }
}

/// Frozen page records of every native source, in source (group) order.
#[derive(Default)]
pub(super) struct Records(HashMap<usize, Vec<PaintRecord>>);

impl PageStack {
    /// Logical-pixel matrix of sheet `i`: the stack scale about `bounds.origin`,
    /// preceded by the sheet's `page-orientation` quarter-turn.
    pub(super) fn sheet_matrix(&self, bounds: Bounds<Pixels>, i: usize) -> TransformationMatrix {
        self.sheet_matrix_turned(bounds, i, self.geom(i).turn)
    }

    fn sheet_matrix_turned(
        &self,
        bounds: Bounds<Pixels>,
        i: usize,
        turn: u8,
    ) -> TransformationMatrix {
        let s = self.grid.get().1;
        let (bx, by) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        let g = self.geom(i);
        let (sx, sy) = self.sheet_origin(i);
        let (ox, oy) = (bx + sx, by + sy);
        let (w, h) = g.size;
        let (rotation_scale, translation) = match turn {
            // rotate-right: p -> O + (H - (y - O.y), x - O.x), then q -> B + s(q - B).
            1 => (
                [[0., -s], [s, 0.]],
                [
                    bx * (1. - s) + s * (ox + h + oy),
                    by * (1. - s) + s * (oy - ox),
                ],
            ),
            // rotate-left: p -> O + (y - O.y, W - (x - O.x)).
            3 => (
                [[0., s], [-s, 0.]],
                [
                    bx * (1. - s) + s * (ox - oy),
                    by * (1. - s) + s * (oy + w + ox),
                ],
            ),
            _ => ([[s, 0.], [0., s]], [bx * (1. - s), by * (1. - s)]),
        };
        TransformationMatrix {
            rotation_scale,
            translation,
        }
    }

    /// Route every OOF fragment of the plan to its sheet and prepaint each native
    /// source once, its hitboxes projected through the unrotated sheet matrix.
    pub(super) fn prepaint_native(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let mut entries = self.native_entries(bounds, window)?;
        entries.sort_by_key(|entry| entry.owner);
        // Rotated sheets keep this unrotated projection (see design: one matrix
        // per prepaint, the source is prepainted once for every sheet).
        let hits = self.sheet_matrix_turned(bounds, usize::MAX, 0);
        let mut sources = Vec::with_capacity(entries.len());
        for entry in entries {
            let native = entry
                .snapshot
                .native
                .clone()
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let slices = entry.admission.nodes(&entry.snapshot);
            let routes: Rc<[PaintSegment]> = Rc::from(entry.routes);
            window.with_hitbox_transform(hits, |window| {
                native.prepaint(routes.clone(), slices, window, cx)
            })?;
            sources.push(native);
        }
        *self.oof.natives.borrow_mut() = sources;
        Ok(())
    }

    fn native_entries(
        &self,
        bounds: Bounds<Pixels>,
        window: &Window,
    ) -> Result<Vec<Entry>, PaintRecordError> {
        let plan = self.oof.plan.borrow();
        if plan.failure.is_some() {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let sources = self.oof.sources.borrow();
        let snapshots = self.oof.snapshots.borrow();
        let repeats = self.oof.repeat_snapshots.borrow();
        let viewport = window.viewport_size();
        let spill = f32::from(viewport.width.max(viewport.height).max(bounds.size.width));
        let mut entries: Vec<Entry> = Vec::new();
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
            let rect = flow.metrics.normal_rect();
            let inline = geometry::inline(self.axis, rect);
            let from = geometry::slice(self.axis, rect, 0., inline, f.from, f.h).origin;
            let page = self.page_area(bounds, f.col);
            let offset = if self.axis.is_vertical() {
                rect.origin.y
            } else {
                rect.origin.x
            };
            let to = geometry::slice(self.axis, page, offset.into(), inline, f.y, f.h).origin;
            // Unbroken edges keep their ink overflow; a break edge cuts it.
            let siblings = || plan.iter().filter(|g| g.kid == f.kid);
            let first = !siblings().any(|g| (g.col, g.from) < (f.col, f.from));
            let last = !siblings().any(|g| (g.col, g.from) > (f.col, f.from));
            let (head, tail) = flow.kid.clone_dec.unwrap_or_default();
            let up = if first { spill } else { -head };
            let down = if last { spill } else { -tail };
            let clip = geometry::slice(
                self.axis,
                rect,
                -spill,
                inline + spill * 2.,
                f.from - up,
                (f.h + up + down).max(0.),
            );
            let index = match entries
                .iter()
                .position(|e| Rc::ptr_eq(&e.snapshot, snapshot))
            {
                Some(index) => index,
                None => {
                    entries.push(Entry::new(*owner, snapshot.clone()));
                    entries.len() - 1
                }
            };
            entries[index].place(flow.id, f.col, from, to, clip)?;
        }
        Ok(entries)
    }

    /// Page area of sheet `i` in unscaled stack coordinates.
    fn page_area(&self, bounds: Bounds<Pixels>, i: usize) -> Bounds<Pixels> {
        let (sx, sy) = self.sheet_origin(i);
        let g = self.geom(i);
        let (ax, ay) = g.area_origin();
        Bounds::new(
            point(bounds.origin.x + px(sx + ax), bounds.origin.y + px(sy + ay)),
            size(px(g.area.0), px(g.area.1)),
        )
    }

    /// The sheet's page area as presented in window coordinates.
    pub(super) fn presented_area(&self, bounds: Bounds<Pixels>, i: usize) -> Bounds<Pixels> {
        let m = self.sheet_matrix(bounds, i);
        let area = self.page_area(bounds, i);
        let (a, z) = (m.apply(area.origin), m.apply(area.bottom_right()));
        Bounds::new(
            point(a.x.min(z.x), a.y.min(z.y)),
            size((z.x - a.x).abs(), (z.y - a.y).abs()),
        )
    }

    /// Paint every native source once into per-sheet immutable records.
    pub(super) fn record_native(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        let natives = std::mem::take(&mut *self.oof.natives.borrow_mut());
        let mut records = Records::default();
        for native in natives {
            for (id, record) in native.record(window, cx)?.into_records() {
                let page = usize::try_from(id.0).map_err(|_| PaintRecordError::InvalidGeometry)?;
                records.0.entry(page).or_default().push(record);
            }
        }
        *self.oof.records.borrow_mut() = records;
        Ok(())
    }

    /// Replay sheet `i` under its presentation matrix, clipped to `clip` (window space).
    pub(super) fn paint_native(
        &self,
        bounds: Bounds<Pixels>,
        i: usize,
        clip: Bounds<Pixels>,
        window: &mut Window,
    ) -> Result<(), PaintRecordError> {
        let records = self.oof.records.borrow();
        let Some(page) = records.0.get(&i) else {
            return Ok(());
        };
        let plan = PaintPlan::sequence(page.iter().cloned().map(PaintPlan::record));
        let origin = point(px(0.), px(0.));
        window
            .with_paint_presentation(self.sheet_matrix(bounds, i), |window| {
                window.paint_plan(
                    &plan,
                    PaintPlacement {
                        source: origin,
                        destination: origin,
                        clip,
                    },
                )
            })?
            .map(|_| ())
    }
}
