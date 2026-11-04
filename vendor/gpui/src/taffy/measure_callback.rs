//! Синхронные листья и вложенные измерения используют один контракт.
use super::*;
use crate::{PaintFuture, PaintStack};
/// Borrowed measurement which may suspend for an intrinsic descendant probe.
pub trait AsyncMeasureFn {
    /// Measure content using the caller's explicit traversal stack.
    fn measure<'a>(
        &'a mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        geometry: MeasuredBox,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut PaintStack,
    ) -> PaintFuture<'a, MeasuredContent>;
}
impl<F> AsyncMeasureFn for F
where
    F: for<'a> AsyncFnMut(
        Size<Option<Pixels>>,
        Size<AvailableSpace>,
        MeasuredBox,
        &'a mut Window,
        &'a mut App,
        &'a mut PaintStack,
    ) -> MeasuredContent,
{
    fn measure<'a>(
        &'a mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        geometry: MeasuredBox,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut PaintStack,
    ) -> PaintFuture<'a, MeasuredContent> {
        Box::pin(self(known, available, geometry, window, cx, stack))
    }
}
pub(super) enum NodeMeasureFn {
    Sync(StackSafe<Box<MeasureFn>>),
    Async(Box<dyn AsyncMeasureFn>),
}
impl NodeMeasureFn {
    pub(super) async fn measure(
        &mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        geometry: MeasuredBox,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> MeasuredContent {
        match self {
            Self::Sync(f) => f(known, available, geometry, window, cx),
            Self::Async(f) => {
                stack
                    .run(|stack| f.measure(known, available, geometry, window, cx, stack))
                    .await
            }
        }
    }
}
