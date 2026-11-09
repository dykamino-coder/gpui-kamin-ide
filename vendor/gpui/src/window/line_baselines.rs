//! Detailed content baseline metadata for fragmentation without changing native first/last APIs.

use super::*;

impl Window {
    /// KaminIDE patch: первая baseline содержимого завершённого border box, без замера.
    pub fn first_content_baseline(&self, layout: LayoutId, axis: crate::Axis) -> Option<Pixels> {
        self.layout_engine.as_ref()?.first_content_baseline(layout, axis, self.scale_factor())
    }

    /// KaminIDE patch: the single in-flow item of a laid-out baseline row and the baseline the
    /// row aligns it by, read from the item's stored layout output (no probe sibling, no extra
    /// layout pass). `None` when the row has no in-flow item.
    pub fn row_item_baseline(&self, row: LayoutId) -> Option<crate::RowItemBaseline> {
        self.layout_engine.as_ref()?.row_item_baseline(row, self.scale_factor())
    }

    /// Measure content with the actual containing block and native resolved padding/border.
    /// The callback returns content size; native layout adds the supplied edges exactly once.
    pub fn request_measured_layout_with_box(
        &mut self,
        style: Style,
        mut measure: impl FnMut(
            Size<Option<Pixels>>, Size<AvailableSpace>, crate::MeasuredBox, &mut Window, &mut App,
        ) -> Size<Pixels> + 'static,
    ) -> LayoutId {
        self.invalidator.debug_assert_prepaint();
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        let scope = self.measure_scope();
        self.layout_engine.as_mut().unwrap().request_measured_layout_with_box(
            style, rem_size, scale_factor,
            move |known, available, geometry, window, cx| scope.enter(window, |window| {
                crate::MeasuredContent::new(measure(known, available, geometry, window, cx))
            }),
        )
    }

    /// KaminIDE patch: measure content with a host measure (`CompactMeasure`), which
    /// `keep_retained_measures` may replace by its compacted form.
    pub fn request_measured_layout_with_measure(&mut self, style: Style, measure: impl crate::CompactMeasure) -> LayoutId {
        self.invalidator.debug_assert_prepaint();
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        let scope = self.measure_scope();
        self.layout_engine.as_mut().unwrap().request_measured_layout_with_measure(
            style, rem_size, scale_factor, Scoped { scope, inner: measure },
        )
    }

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
        mut measure: impl FnMut(
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
        let scope = self.measure_scope();
        self.layout_engine
            .as_mut()
            .unwrap()
            .request_measured_layout_with_line_baselines(style, rem_size, scale_factor, move |known, available, window, cx| {
                scope.enter(window, |window| measure(known, available, window, cx))
            })
    }

    /// Measure actual first/last content baselines on both physical axes.
    pub fn request_measured_layout_with_physical_baselines(
        &mut self,
        style: Style,
        mut measure: impl FnMut(
            Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App,
        ) -> crate::MeasuredContent + 'static,
    ) -> LayoutId {
        self.invalidator.debug_assert_prepaint();
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        let scope = self.measure_scope();
        self.layout_engine.as_mut().unwrap()
            .request_measured_layout_with_physical_baselines(style, rem_size, scale_factor, move |known, available, window, cx| {
                scope.enter(window, |window| measure(known, available, window, cx))
            })
    }

}

/// KaminIDE patch: a host measure run in the text style scope of its request.
struct Scoped<M> {
    scope: super::text_style_scope::MeasureScope,
    inner: M,
}

impl<M: crate::CompactMeasure> crate::Measure for Scoped<M> {
    fn measure(
        &mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        geometry: crate::MeasuredBox,
        window: &mut Window,
        cx: &mut App,
    ) -> crate::MeasuredContent {
        let inner = &mut self.inner;
        self.scope.enter(window, |window| inner.measure(known, available, geometry, window, cx))
    }

    fn compacted(&mut self) -> Option<Box<dyn crate::Measure>> {
        let inner = self.inner.compact()?;
        Some(Box::new(Scoped { scope: self.scope.clone(), inner }))
    }
}