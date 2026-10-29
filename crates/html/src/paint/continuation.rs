//! Место absolute/fixed корня в legacy-копии предков: копия раскладывает его как раньше,
//! но рисует его собственную запись продолжения в своём сегменте — внутри того же
//! экземпляра эффектов предков и в CSS-порядке своего контекста наложения.

use crate::hooks::{BareRecords, IntoHooked, SlotScopes, Wrap};
use gpui::{
    AnyElement, App, Bounds, PaintPlacement, PaintPlan, PaintSegmentId, Pixels, Point,
    TransformationMatrix, Window, point, px,
};
use std::rc::Rc;
pub(crate) mod replayed;

struct CopyHits {
    segment: PaintSegmentId,
    scopes: SlotScopes,
}

/// Prepaint a legacy copy for `segment`: its slots report their hit scope.
pub(crate) fn with_copy_hits<R>(
    segment: PaintSegmentId,
    scopes: &SlotScopes,
    window: &mut Window,
    prepaint: impl FnOnce(&mut Window) -> R,
) -> R {
    let context = CopyHits {
        segment,
        scopes: scopes.clone(),
    };
    window.with_render_context(Some(Rc::new(context)), prepaint)
}

/// The segment painted by the legacy copy being painted and its own records.
pub(crate) struct CopySegment {
    pub(crate) segment: PaintSegmentId,
    pub(crate) records: BareRecords,
}

/// Paint a legacy copy for `segment`; deferred positioned paint keeps this
/// context in its paint snapshot.
pub(crate) fn with_copy_segment(
    segment: PaintSegmentId,
    records: Option<&BareRecords>,
    window: &mut Window,
    paint: impl FnOnce(&mut Window),
) {
    match records {
        Some(records) => window.with_render_context(
            Some(Rc::new(CopySegment {
                segment,
                records: records.clone(),
            })),
            paint,
        ),
        None => paint(window),
    }
}

/// The clip of a fragmentainer whose copy is cut to its own slice: a continued
/// root replayed in that copy is cut by its fragmentainer, not by the slice
/// of the box that holds it (a table fragment ends above its repeated footer).
struct CopyArea(gpui::ContentMask<Pixels>);

/// Paint a sliced copy whose slots replay under `area` (window coordinates,
/// as `Window::content_mask` reports them).
pub(crate) fn with_copy_area(
    area: gpui::ContentMask<Pixels>,
    window: &mut Window,
    paint: impl FnOnce(&mut Window),
) {
    window.with_render_context(Some(Rc::new(CopyArea(area))), paint)
}

pub(crate) fn slot(id: Option<u64>, child: AnyElement) -> AnyElement {
    match id {
        Some(id) => Slot { id, child }.hooked(),
        None => child,
    }
}

struct Slot {
    id: u64,
    child: AnyElement,
}

impl Wrap for Slot {
    fn paint_boundary(&self) -> bool {
        false
    }
    // The positioned root keeps its paint phase (CSS 2.1 Appendix E): the replay
    // joins the positioned phase of the copy's stacking context, after the flow
    // of every fragmentainer, even behind the copy's opaque consumers.
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        Some(&crate::paint::coordinator::REPLAY)
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        // The native placement replayed here is hit through this copy's scope.
        match window.render_context::<CopyHits>() {
            Some(copy) if copy.scopes.natives().contains(self.id) => {
                // The recorded main copy holds every fragmentainer of its pieces.
                for segment in
                    std::iter::once(copy.segment).chain(replayed::columns(window, copy.segment))
                {
                    copy.scopes.push(segment, self.id, window.paint_hit_scope());
                }
            }
            // A root no native source replays stays with its copy.
            _ => {
                self.child.prepaint(window, cx);
                return;
            }
        }
        // Layout stays with the copy; its hitboxes live at the native placements
        // only (a singular matrix makes this copy's own ones unreachable).
        let hidden = TransformationMatrix {
            rotation_scale: [[0., 0.], [0., 0.]],
            translation: [0., 0.],
        };
        window.with_hitbox_transform(hidden, |window| self.child.prepaint(window, cx));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let copy = window.render_context::<CopySegment>();
        let Some(copy) = copy.filter(|copy| copy.records.natives().contains(self.id)) else {
            if !replayed::paint(self.id, window) {
                self.child.paint(window, cx);
            }
            return;
        };

        let records = copy.records.get(copy.segment, self.id);
        let origin = point(px(0.), px(0.));
        if records.is_empty() {
            return;
        }
        match window.render_context::<CopyArea>() {
            Some(area) => {
                window.with_content_mask_exact(area.0, |window| replay(records, origin, window))
            }
            None => replay(records, origin, window),
        }
    }
}

/// Replay `records` moved by `shift` (inside a recording: back to its source).
fn replay(records: Vec<crate::hooks::Replay>, shift: Point<Pixels>, window: &mut Window) {
    {
        // The copy's own transform (CSS transforms of the copied ancestors, the
        // page sheet) presents the replayed records; the root's own per-fragment
        // transform applies first.
        let device = window.current_transformation();
        let k = window.scale_factor();
        let copy = (device != TransformationMatrix::unit()).then(|| TransformationMatrix {
            rotation_scale: device.rotation_scale,
            translation: [device.translation[0] / k, device.translation[1] / k],
        });
        // The shift applies under the copy's transform and before the root's own.
        let moved = TransformationMatrix {
            rotation_scale: [[1., 0.], [0., 1.]],
            translation: [f32::from(shift.x), f32::from(shift.y)],
        };
        let copy = match copy {
            _ if moved == TransformationMatrix::unit() => copy,
            Some(matrix) => Some(matrix.compose(moved)),
            None => Some(moved),
        };
        for (record, own) in records {
            let origin = point(px(0.), px(0.));
            let placement = PaintPlacement {
                source: origin,
                destination: origin,
                clip: window.content_mask().bounds,
            };
            let plan = PaintPlan::record(record);
            let replay = |window: &mut Window| match own {
                Some(matrix) => window
                    .with_paint_presentation(matrix, |window| window.paint_plan(&plan, placement))
                    .and_then(|result| result.map(|_| ())),
                None => window.paint_plan(&plan, placement).map(|_| ()),
            };
            // A stale or foreign record is rejected without output.
            let _ = match copy {
                Some(matrix) => window
                    .with_paint_presentation(matrix, replay)
                    .and_then(|result| result),
                None => replay(window),
            };
        }
    }
}
