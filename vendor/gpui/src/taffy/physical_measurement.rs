//! Read both completed physical baseline sets from independently measured subtrees.

use super::*;
use crate::MeasuredContent;

impl LayoutMeasurement {
    /// Measure size and both physical first/last sets without device-pixel rounding.
    /// Complete line metadata is retained in the subtree for fragment consumers.
    pub fn measure_physical_baselines(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> MeasuredContent {
        reblessive::Stack::new()
            .enter(|stack| self.measure_physical_baselines_async(available, window, cx, stack))
            .finish()
    }
}

impl LayoutMeasurement {
    /// Измеряет поддерево на общем стеке продолжений без роста стека потока.
    pub async fn measure_physical_baselines_async(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) -> MeasuredContent {
        let (size, first_y, last_y) = self.measure_async(available, window, cx, stack).await;
        let output = self
            .tree
            .computed_layout_output(self.root)
            .expect(EXPECT_MESSAGE)
            .expect("the subtree completed full layout");
        MeasuredContent {
            first_y,
            last_y,
            first_x: output
                .baselines_x
                .first
                .map(|value| Pixels(value / self.scale_factor)),
            last_x: output
                .baselines_x
                .last
                .map(|value| Pixels(value / self.scale_factor)),
            ..MeasuredContent::new(size)
        }
    }
}
