//! Продолжение Div сохраняет порядок и области дочерней фазы request_layout.

use super::*;

impl Div {
    pub(super) async fn request_layout_on_stack(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) -> (LayoutId, DivFrameState) {
        let mut child_layout_ids = SmallVec::new();
        let image_cache = self
            .image_cache
            .as_mut()
            .map(|provider| provider.provide(window, cx));

        let layout_id = window
            .with_image_cache_async(image_cache, async |window| {
                self.interactivity
                    .request_layout_boxed_async(
                        global_id,
                        inspector_id,
                        window,
                        cx,
                        stack,
                        async |style, refinement, window, cx, stack| {
                            window
                                .with_text_style_async(
                                    style.text_style().cloned(),
                                    async |window| {
                                        for child in &mut self.children {
                                            child_layout_ids.push(
                                                child.request_layout_async(window, cx, stack).await,
                                            );
                                        }
                                        let children = child_layout_ids.iter().copied();
                                        match refinement {
                                            Some(refinement) => window.request_layout_refined(
                                                &style, refinement, children, cx,
                                            ),
                                            None => window.request_layout_ref(&style, children, cx),
                                        }
                                    },
                                )
                                .await
                        },
                    )
                    .await
            })
            .await;

        // KaminIDE patch: capture identity in the tree receiving this node.
        if let Some(observer) = &mut self.layout_observer {
            observer.request_layout(layout_id, window);
        }
        (
            layout_id,
            DivFrameState {
                child_layout_ids,
                layout_id: Some(layout_id),
                exact_bounds: None,
            },
        )
    }
}
