//! Публикация построенного у CB элемента для отложенной покраски в исходном месте дерева.

use gpui::{AnyElement, ContentMask, Pixels};
use std::{cell::RefCell, rc::Rc};

type DeferredChild = (AnyElement, ContentMask<Pixels>);

#[derive(Clone, Default)]
pub(crate) struct DeferredPaint(Rc<RefCell<Option<DeferredChild>>>);

impl DeferredPaint {
    pub(crate) fn publish(&self, child: AnyElement, mask: ContentMask<Pixels>) {
        *self.0.borrow_mut() = Some((child, mask));
    }

    /// Доступ к текущему элементу с удержанием заимствования общего слота.
    #[expect(
        clippy::await_holding_refcell_ref,
        reason = "exclusive slot access spans same-thread PaintStack child polls; no executor yield"
    )]
    pub(crate) async fn with_child(
        &self,
        paint: impl AsyncFnOnce(&mut AnyElement, ContentMask<Pixels>),
    ) {
        if let Some((child, mask)) = self.0.borrow_mut().as_mut() {
            paint(child, *mask).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use gpui::{IntoElement, div};
    use std::panic::AssertUnwindSafe;

    #[test]
    fn child_slot_stays_exclusive_through_continuations_and_releases_on_unwind() {
        let slot = DeferredPaint::default();
        slot.publish(div().into_any_element(), ContentMask::default());
        reblessive::Stack::new()
            .enter(|stack| async {
                let result = AssertUnwindSafe(slot.with_child(async |_, _| {
                    stack
                        .run(|_| async {
                            assert!(slot.0.try_borrow_mut().is_err());
                        })
                        .await;
                    panic!("abort deferred child");
                }))
                .catch_unwind()
                .await;
                assert!(result.is_err());
                assert!(slot.0.try_borrow_mut().unwrap().is_some());
                slot.with_child(async |_, _| {}).await;
            })
            .finish();
    }
}
