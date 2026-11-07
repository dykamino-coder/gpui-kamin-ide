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
        window.continuations().pin(self(known, available, geometry, window, cx, stack))
    }
}
pub(super) enum NodeMeasureFn {
    Sync(StackSafe<Box<MeasureFn>>),
    Async(Box<dyn AsyncMeasureFn>),
    Active,
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
            Self::Active => panic!("reentrant measurement of the same node"),
            Self::Sync(f) => f(known, available, geometry, window, cx),
            Self::Async(f) => {
                stack
                    .run(|stack| f.measure(known, available, geometry, window, cx, stack))
                    .await
            }
        }
    }
}

// The callback is owned across suspension; no RefCell borrow survives an await.
pub(super) struct MeasureGuard<'a> {
    slot: &'a std::cell::RefCell<NodeMeasureFn>,
    callback: Option<NodeMeasureFn>,
}
impl<'a> MeasureGuard<'a> {
    pub(super) fn take(slot: &'a std::cell::RefCell<NodeMeasureFn>) -> Self {
        let mut value = slot.borrow_mut();
        assert!(
            !matches!(*value, NodeMeasureFn::Active),
            "reentrant measurement of the same node"
        );
        let callback = Some(std::mem::replace(&mut *value, NodeMeasureFn::Active));
        Self { slot, callback }
    }
}
impl std::ops::Deref for MeasureGuard<'_> {
    type Target = NodeMeasureFn;
    fn deref(&self) -> &Self::Target {
        self.callback.as_ref().unwrap()
    }
}
impl std::ops::DerefMut for MeasureGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.callback.as_mut().unwrap()
    }
}
impl Drop for MeasureGuard<'_> {
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.callback.take().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suspended_callback_is_restored_on_cancellation() {
        let slot = std::cell::RefCell::new(NodeMeasureFn::Sync(StackSafe::new(Box::new(
            |_, _, _, _, _| MeasuredContent::new(Size::default()),
        ))));
        {
            let _guard = MeasureGuard::take(&slot);
            assert!(matches!(*slot.borrow(), NodeMeasureFn::Active));
        }
        assert!(matches!(*slot.borrow(), NodeMeasureFn::Sync(_)));
    }
}
