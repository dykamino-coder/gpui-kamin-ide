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
        let (size, first_y, last_y) = self.measure(available, window, cx);
        let output = self.tree.computed_layout_output(self.root)
            .expect(EXPECT_MESSAGE).expect("the subtree completed full layout");
        MeasuredContent {
            first_y,
            last_y,
            first_x: output.baselines_x.first.map(|value| Pixels(value / self.scale_factor)),
            last_x: output.baselines_x.last.map(|value| Pixels(value / self.scale_factor)),
            ..MeasuredContent::new(size)
        }
    }
}
