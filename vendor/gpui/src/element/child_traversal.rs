//! Потомки передают управление явному стеку между уровнями дерева.

use super::*;
use futures::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe};

/// Forward an unwind through the suspended caller, just like a native call.
/// The scheduler polls child tasks separately, outside the caller's catch_unwind.
pub(super) async fn run_child<'a, F, Fut, R>(stack: &'a mut PaintStack, child: F) -> R
where
    F: FnOnce(&'a mut PaintStack) -> Fut + 'a,
    Fut: Future<Output = R> + 'a,
    R: 'a,
{
    let result = stack
        .run(|stack| async move {
            AssertUnwindSafe(async move { child(stack).await })
                .catch_unwind()
                .await
        })
        .await;
    match result {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

impl AnyElement {
    /// Планирует запрос потомка, сохраняя native стек ограниченным.
    pub async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> LayoutId {
        run_child(stack, |stack| self.0.request_layout_async(window, cx, stack)).await
    }
    /// Планирует prepaint потомка с сохранением focus-протокола.
    pub async fn prepaint_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Option<FocusHandle> {
        let focus_assigned = window.next_frame.focus.is_some();

        run_child(stack, |stack| self.0.prepaint_async(window, cx, stack)).await;

        if !focus_assigned && let Some(focus_id) = window.next_frame.focus {
            return FocusHandle::for_id(focus_id, &cx.focus_handles);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    struct Scope(usize, Rc<RefCell<Vec<usize>>>);
    impl Drop for Scope {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }

    fn abort_at_leaf(
        stack: &mut PaintStack,
        depth: usize,
        dropped: Rc<RefCell<Vec<usize>>>,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + '_>> {
        Box::pin(async move {
            let _scope = Scope(depth, dropped.clone());
            if depth == 0 {
                panic::panic_any(42_u32);
            }
            run_child(stack, |stack| abort_at_leaf(stack, depth - 1, dropped)).await;
        })
    }

    #[test]
    fn deep_child_panic_reaches_caller_and_drops_scopes_in_order() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let dropped = Rc::new(RefCell::new(Vec::new()));
                reblessive::Stack::new()
                    .enter(|stack| async {
                        let result = AssertUnwindSafe(abort_at_leaf(stack, 20_000, dropped.clone()))
                            .catch_unwind()
                            .await;
                        assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 42);
                        assert!(dropped.borrow().iter().copied().eq(0..=20_000));
                        assert_eq!(run_child(stack, |_| async { 17 }).await, 17);
                    })
                    .finish();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
