//! Место absolute/fixed корня в legacy-копии предков: копия раскладывает его как раньше,
//! но рисует его собственную запись продолжения в своём сегменте — внутри того же
//! экземпляра эффектов предков и в CSS-порядке своего контекста наложения.

use crate::hooks::{BareRecords, IntoHooked, Wrap};
use gpui::{
    AnyElement, App, Bounds, PaintPlacement, PaintPlan, PaintSegmentId, Pixels,
    TransformationMatrix, Window, point, px,
};
use std::rc::Rc;

/// The segment painted by the legacy copy being painted and its own records.
pub(crate) struct CopySegment {
    pub(crate) segment: PaintSegmentId,
    pub(crate) records: BareRecords,
    /// Presentation of the copy relative to the source capture (a page sheet).
    pub(crate) presentation: Option<TransformationMatrix>,
}

/// Paint a legacy copy for `segment`; deferred positioned paint keeps this
/// context in its paint snapshot.
pub(crate) fn with_copy_segment(
    segment: PaintSegmentId,
    records: Option<&BareRecords>,
    presentation: Option<TransformationMatrix>,
    window: &mut Window,
    paint: impl FnOnce(&mut Window),
) {
    match records {
        Some(records) => window.with_render_context(
            Some(Rc::new(CopySegment {
                segment,
                records: records.clone(),
                presentation,
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
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
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
            eprintln!("P08DBG slot {} no copy context", self.id);
            return;
        };
        eprintln!(
            "P08DBG slot {} seg {:?} found {}",
            self.id,
            copy.segment,
            copy.records.get(copy.segment, self.id).is_some()
        );
        let record = copy.records.get(copy.segment, self.id);
        if let Some(record) = record {
            let origin = point(px(0.), px(0.));
            let placement = PaintPlacement {
                source: origin,
                destination: origin,
                clip: window.content_mask().bounds,
            };
            eprintln!(
                "P08DBG slot replay mask {:?} quads {:?}",
                window.content_mask().bounds,
                record
                    .scene()
                    .quads
                    .iter()
                    .map(|q| (
                        q.bounds,
                        q.content_mask.bounds,
                        q.transformation.translation
                    ))
                    .collect::<Vec<_>>()
            );
            let plan = PaintPlan::record(record);
            // A stale or foreign record is rejected without output.
            let _ = match copy.presentation {
                Some(matrix) => window
                    .with_paint_presentation(matrix, |window| window.paint_plan(&plan, placement))
                    .map(|_| ()),
                None => window.paint_plan(&plan, placement).map(|_| ()),
            };
        }
    }
}
