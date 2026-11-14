//! Неизменяемые команды одного draw и явное размещение в другом paint scope.

use super::*;
mod invocation;
mod plan;
mod validation;
pub(super) mod segments;
pub use plan::{PaintGroupDescriptor, PaintPlan, PaintScopeDescriptor};
pub use segments::{PaintSegment, PaintSegmentId, PaintSegmentRecords, PaintSegmentClips, PaintSegmentSlice};

/// Immutable drawing commands. Clones share commands and retained resources.
/// Capture scope may close; the original window, draw and raster scale must live.
#[derive(Clone)]
pub struct PaintRecord(Rc<Recorded>);

struct Recorded {
    identity: paint_scope::Identity,
    scale: f32,
    clip: Bounds<Pixels>,
    /// KaminIDE patch: the frozen commands; of a segment kept as a view of
    /// its draft, made only when something reads them as a scene.
    scene: std::cell::OnceCell<Scene>,
    segment: Option<Segment>,
}

/// KaminIDE patch: a segment of a draft shared by the records of all its
/// segments (`Window::record_paint_segments`): the picks of the draft's
/// commands, placed when the record is painted.
struct Segment {
    draft: Rc<Scene>,
    id: PaintSegmentId,
    source: Bounds<Pixels>,
    view: crate::scene::segments::SegmentView,
}

impl Recorded {
    fn frozen(identity: paint_scope::Identity, scale: f32, clip: Bounds<Pixels>, scene: Scene) -> Self {
        Self { identity, scale, clip, scene: scene.into(), segment: None }
    }

    fn scene(&self) -> &Scene {
        self.scene.get_or_init(|| {
            let segment = self.segment.as_ref().expect("a record is a scene or a segment of a draft");
            segment.draft.freeze_segment(segment.id, self.scale, segment.source)
        })
    }
}

/// Explicit source-to-destination translation and final destination-space clip.
/// Captured transforms are already resolved; caller transforms are not reapplied.
#[derive(Clone, Copy, Debug)]
pub struct PaintPlacement {
    /// Anchor in the recorded source coordinate system.
    pub source: Point<Pixels>,
    /// Destination of the source anchor, in the target window coordinate system.
    pub destination: Point<Pixels>,
    /// Clip after the complete plan's effects, in destination coordinates.
    pub clip: Bounds<Pixels>,
}

/// A rejected record or placement never changes the destination scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PaintRecordError {
    /// Commands can only be captured or invoked during paint.
    #[error("paint record operation is outside the paint phase")]
    NotPainting,
    /// Record resources belong to another or completed window draw.
    #[error(transparent)]
    Identity(#[from] PaintError),
    /// Rasterized resources cannot be reinterpreted at another device scale.
    #[error("paint record raster scale differs from the destination")]
    ScaleMismatch,
    /// Geometry contains nonfinite coordinates, sizes or effect parameters.
    #[error("paint record geometry is invalid")]
    InvalidGeometry,
    /// Выбран необъявленный сегмент либо сегментная запись не активна.
    #[error("paint segment is outside the active recording boundary")]
    InvalidSegment,
}

impl PaintRecord {
    /// Whether two handles share the same immutable command storage.
    pub fn same_record(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) }

    /// Inspect frozen commands in renderer contract tests without mutating them.
    #[cfg(any(test, feature = "test-support"))]
    pub fn scene(&self) -> &Scene { self.0.scene() }

    /// Проверка сохранённых подложек до включения их команд в кадр.
    #[cfg(any(test, feature = "test-support"))]
    pub fn hoisted_backdrops(&self) -> &[crate::PaintSurface] { &self.0.scene().hoisted_backdrops }

    fn check(&self, window: &Window) -> Result<(), PaintRecordError> {
        self.0.identity.check_pass(window)?;
        if self.0.scale != window.scale_factor() {
            return Err(PaintRecordError::ScaleMismatch);
        }
        Ok(())
    }
}
