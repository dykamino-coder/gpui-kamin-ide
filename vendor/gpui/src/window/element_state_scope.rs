//! Единая передача persistent state синхронным областям и продолжениям обхода.

use super::*;

pub(super) struct ElementStateSlot<S> {
    key: (GlobalElementId, TypeId),
    state_box: Option<Box<Option<S>>>,
    #[cfg(debug_assertions)]
    type_name: &'static str,
}

impl<S: 'static> ElementStateSlot<S> {
    pub(super) fn restore(self, state: S, window: &mut Window) {
        let inner = match self.state_box {
            Some(mut state_box) => {
                state_box.replace(state);
                state_box
            }
            None => Box::new(Some(state)),
        };
        window.insert_element_state(
            self.key,
            ElementStateBox {
                inner,
                #[cfg(debug_assertions)]
                type_name: self.type_name,
            },
        );
    }
}

impl Window {
    pub(super) fn take_typed_element_state<S: 'static>(
        &mut self,
        global_id: &GlobalElementId,
    ) -> (Option<S>, ElementStateSlot<S>) {
        self.invalidator.debug_assert_paint_or_prepaint();
        let (key, state) = self.take_element_state(global_id, TypeId::of::<S>());
        let mut slot = ElementStateSlot {
            key,
            state_box: None,
            #[cfg(debug_assertions)]
            type_name: std::any::type_name::<S>(),
        };
        if let Some(any) = state {
            let ElementStateBox {
                inner,
                #[cfg(debug_assertions)]
                type_name,
            } = any;
            let state_box = inner
                .downcast::<Option<S>>()
                .map_err(|_| {
                    #[cfg(debug_assertions)]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}, actual: {:?}",
                            std::any::type_name::<S>(),
                            type_name
                        )
                    }

                    #[cfg(not(debug_assertions))]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}",
                            std::any::type_name::<S>(),
                        )
                    }
                })
                .unwrap();

            slot.state_box = Some(state_box);
            #[cfg(debug_assertions)]
            {
                slot.type_name = type_name;
            }
            let state = slot.state_box.as_mut().unwrap().take().expect(
                "reentrant call to with_element_state for the same state type and element id",
            );
            (Some(state), slot)
        } else {
            (None, slot)
        }
    }

    /// Выполняет область состояния с продолжением на явном стеке обхода.
    pub async fn with_element_state_async<S: 'static, R>(
        &mut self,
        global_id: &GlobalElementId,
        f: impl AsyncFnOnce(Option<S>, &mut Self) -> (R, S),
    ) -> R {
        let (state, slot) = self.take_typed_element_state::<S>(global_id);
        let (result, state) = f(state, self).await;
        slot.restore(state, self);
        result
    }
}
