//! Измерение листа может приостановить обход для вложенной раскладки.
use crate::{LayoutInput, LayoutOutput, NodeId, Style};
use core::future::Future;
/// KaminIDE patch: borrowed measurement on the caller's explicit stack.
pub trait LayoutMeasurer<Context> {
    /// Measure one leaf; references remain valid until this continuation finishes.
    fn measure<'a>(
        &'a mut self,
        input: LayoutInput,
        node: NodeId,
        context: Option<&'a mut Context>,
        style: &'a Style,
        stack: &'a mut reblessive::Stk,
    ) -> impl Future<Output = LayoutOutput> + 'a;
}
/// KaminIDE patch: adapt an existing synchronous leaf callback.
pub struct SyncMeasure<F>(pub F);
impl<C, F> LayoutMeasurer<C> for SyncMeasure<F>
where
    F: FnMut(LayoutInput, NodeId, Option<&mut C>, &Style) -> LayoutOutput,
{
    async fn measure<'a>(
        &'a mut self,
        input: LayoutInput,
        node: NodeId,
        context: Option<&'a mut C>,
        style: &'a Style,
        _stack: &'a mut reblessive::Stk,
    ) -> LayoutOutput {
        (self.0)(input, node, context, style)
    }
}
/// KaminIDE patch: adapt a lending asynchronous leaf callback.
pub struct AsyncMeasure<F>(pub F);
impl<C, F> LayoutMeasurer<C> for AsyncMeasure<F>
where
    F: for<'a> AsyncFnMut(
        LayoutInput,
        NodeId,
        Option<&'a mut C>,
        &'a Style,
        &'a mut reblessive::Stk,
    ) -> LayoutOutput,
{
    async fn measure<'a>(
        &'a mut self,
        input: LayoutInput,
        node: NodeId,
        context: Option<&'a mut C>,
        style: &'a Style,
        stack: &'a mut reblessive::Stk,
    ) -> LayoutOutput {
        (self.0)(input, node, context, style, stack).await
    }
}
