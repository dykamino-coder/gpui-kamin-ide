//! Запись диапазонов retained охватывает всё дочернее продолжение.

use super::*;

impl Window {
    /// Run the prepaint of a subtree and return its range in this frame.
    pub fn record_retained_prepaint<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, RetainedPrepaint) {
        let start = self.prepaint_index();
        let result = f(self);
        (result, RetainedPrepaint(start..self.prepaint_index()))
    }

    /// Run the paint of a subtree and return its range in this frame.
    pub fn record_retained_paint<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, RetainedPaint) {
        let root = self.retained_root_environment();
        let groups = self.next_frame.scene.groups.len();
        let start = self.paint_index();
        let result = f(self);
        let replayable = root && groups == self.next_frame.scene.groups.len();
        (
            result,
            RetainedPaint {
                range: start..self.paint_index(),
                replayable,
            },
        )
    }

    /// Run the prepaint of a subtree and return its range in this frame.
    pub async fn record_retained_prepaint_async<R>(
        &mut self,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> (R, RetainedPrepaint) {
        let start = self.prepaint_index();
        let result = f(self).await;
        (result, RetainedPrepaint(start..self.prepaint_index()))
    }

    /// Run the paint of a subtree and return its range in this frame.
    pub async fn record_retained_paint_async<R>(
        &mut self,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> (R, RetainedPaint) {
        let root = self.retained_root_environment();
        let groups = self.next_frame.scene.groups.len();
        let start = self.paint_index();
        let result = f(self).await;
        let replayable = root && groups == self.next_frame.scene.groups.len();
        (
            result,
            RetainedPaint {
                range: start..self.paint_index(),
                replayable,
            },
        )
    }
}
