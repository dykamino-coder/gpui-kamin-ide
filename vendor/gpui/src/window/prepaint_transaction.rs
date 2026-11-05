//! Пробный prepaint откатывает те же записи при синхронном и lending-вызове.
use super::*;

struct Transaction<'a> {
    window: &'a mut Window,
    index: Option<PrepaintStateIndex>,
}
impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if let Some(index) = self.index.take() {
            self.window.rollback_prepaint(index);
        }
    }
}
impl Window {
    pub(super) fn rollback_prepaint(&mut self, index: PrepaintStateIndex) {
        self.next_frame.hitboxes.truncate(index.hitboxes_index);
        self.next_frame
            .hitbox_entry_scopes
            .retain(|entry, _| *entry < index.hitboxes_index);
        self.next_frame
            .tooltip_requests
            .truncate(index.tooltips_index);
        self.next_frame
            .deferred_draws
            .truncate(index.deferred_draws_index);
        self.next_frame
            .dispatch_tree
            .truncate(index.dispatch_tree_index);
        self.next_frame
            .accessed_element_states
            .truncate(index.accessed_element_states_index);
        self.text_system.truncate_layouts(index.line_layout_index);
    }
    /// Perform a retryable prepaint while sharing its descendant traversal stack.
    pub async fn transact_async<T, U>(
        &mut self,
        f: impl AsyncFnOnce(&mut Self) -> Result<T, U>,
    ) -> Result<T, U> {
        self.invalidator.debug_assert_prepaint();
        let index = Some(self.prepaint_index());
        let mut transaction = Transaction {
            window: self,
            index,
        };
        let result = f(transaction.window).await;
        if result.is_ok() {
            transaction.index = None;
        }
        result
    }
}
