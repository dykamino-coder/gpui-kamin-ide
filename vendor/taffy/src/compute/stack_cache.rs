//! Кэш раскладки завершает запись после всех продолжений потомка.
use super::*;
pub(crate) async fn compute_cached_layout_async<Tree: CacheTree + ?Sized>(
    tree: &mut Tree,
    node: NodeId,
    inputs: LayoutInput,
    compute_uncached: impl AsyncFnOnce(
        &mut Tree,
        NodeId,
        LayoutInput,
        &mut reblessive::Stk,
    ) -> LayoutOutput,
    stack: &mut reblessive::Stk,
) -> LayoutOutput {
    debug_push_node!(node);
    if let Some(cached) = tree.cache_get(node, &inputs) {
        debug_log_node!(inputs);
        debug_log!("RESULT (CACHED)", dbg:cached.size);
        debug_pop_node!();
        return cached;
    }
    debug_log_node!(inputs);
    let output = compute_uncached(tree, node, inputs, stack).await;
    store_cached_layout(tree, node, &inputs, &output);
    debug_log!("RESULT", dbg:output.size);
    debug_pop_node!();
    output
}
