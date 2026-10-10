//! Detailed content baseline metadata for fragmentation without changing native first/last APIs.

use super::*;

impl Window {
    /// Copy a subtree for independent measurement during the current layout phase.
    pub fn snapshot_layout_measurement(&self, layout_id: LayoutId) -> crate::LayoutMeasurement {
        self.layout_engine
            .as_ref()
            .unwrap()
            .snapshot_measurement(layout_id, self.scale_factor())
    }

    /// Measure content and expose every actual line baseline for visible-fragment consumers.
    pub fn request_measured_layout_with_line_baselines(
        &mut self,
        style: Style,
        measure: impl FnMut(
            Size<Option<Pixels>>,
            Size<AvailableSpace>,
            &mut Window,
            &mut App,
        ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>, Vec<Pixels>)
        + 'static,
    ) -> LayoutId {
        self.invalidator.debug_assert_prepaint();
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        self.layout_engine
            .as_mut()
            .unwrap()
            .request_measured_layout_with_line_baselines(style, rem_size, scale_factor, measure)
    }

    /// Measure actual first/last content baselines on both physical axes.
    pub fn request_measured_layout_with_physical_baselines(
        &mut self,
        style: Style,
        measure: impl FnMut(
            Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App,
        ) -> crate::MeasuredContent + 'static,
    ) -> LayoutId {
        self.invalidator.debug_assert_prepaint();
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        self.layout_engine.as_mut().unwrap()
            .request_measured_layout_with_physical_baselines(style, rem_size, scale_factor, measure)
    }

}
