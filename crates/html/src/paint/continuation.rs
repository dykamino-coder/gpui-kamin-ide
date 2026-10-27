//! Место absolute/fixed корня в legacy-копии предков: копия раскладывает его как раньше,
//! но рисует его собственную запись продолжения в своём сегменте — внутри того же
//! экземпляра эффектов предков и в CSS-порядке своего контекста наложения.

use crate::hooks::{BareRecords, IntoHooked, SlotScopes, Wrap};
use gpui::{
    AnyElement, App, Bounds, PaintPlacement, PaintPlan, PaintSegmentId, Pixels,
    TransformationMatrix, Window, point, px,
};
use std::rc::Rc;

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
    // The positioned root keeps its paint phase (CSS 2.1 Appendix E): the
    // replay happens where the copy would have painted the box.
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        self.child.paint_metadata()
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        // The native placement replayed here is hit through this copy's scope.
        if let Some(copy) = window.render_context::<CopyHits>() {
            copy.scopes
                .push(copy.segment, self.id, window.paint_hit_scope());
        }
        // Layout stays with the copy; its hitboxes live at the native placements
        // only (a singular matrix makes this copy's own ones unreachable).
        let hidden = TransformationMatrix {
            rotation_scale: [[0., 0.], [0., 0.]],
            translation: [0., 0.],
        };
        window.with_hitbox_transform(hidden, |window| self.child.prepaint(window, cx));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, _: &mut App) {
        let Some(copy) = window.render_context::<CopySegment>() else {
            return;
        };
        let records = copy.records.get(copy.segment, self.id);
        if records.is_empty() {
            return;
        }
        // The copy's own transform (CSS transforms of the copied ancestors, the
        // page sheet) presents the replayed records; the root's own per-fragment
        // transform applies first.
        let device = window.current_transformation();
        let k = window.scale_factor();
        let copy = (device != TransformationMatrix::unit()).then(|| TransformationMatrix {
            rotation_scale: device.rotation_scale,
            translation: [device.translation[0] / k, device.translation[1] / k],
        });
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
