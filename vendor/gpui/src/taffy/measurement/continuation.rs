//! Независимый щуп продолжает общий стек вложенных измерений.
use super::*;
impl LayoutMeasurement {
    /// Измеряет поддерево на общем стеке продолжений без роста стека потока.
    pub async fn measure_async(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        let scale = self.scale_factor;
        let available = taffy::Size {
            width: to_native(available.width, scale),
            height: to_native(available.height, scale),
        };
        self.tree
            .compute_layout_with_measure_async(
                self.root,
                available,
                taffy::tree::AsyncMeasure(
                    async |input: taffy::LayoutInput,
                           _: taffy::NodeId,
                           context: Option<&mut crate::taffy::NodeContext>,
                           style: &taffy::Style,
                           stack: &mut crate::PaintStack| {
                        leaf_async(input, context, style, scale, window, cx, stack).await
                    },
                ),
                stack,
            )
            .await
            .expect(EXPECT_MESSAGE);
        let output = self
            .tree
            .computed_layout_output(self.root)
            .expect(EXPECT_MESSAGE)
            .expect("the subtree completed full layout");
        (
            size(
                Pixels(output.size.width / scale),
                Pixels(output.size.height / scale),
            ),
            output.baselines.first.map(|value| Pixels(value / scale)),
            output.baselines.last.map(|value| Pixels(value / scale)),
        )
    }
}
