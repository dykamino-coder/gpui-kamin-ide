//! Корень и его измерения приостанавливаются на общем явном стеке.
use super::*;
struct LayoutEngineRestore<'a> {
    window: &'a mut Window,
    engine: Option<TaffyLayoutEngine>,
}
impl Drop for LayoutEngineRestore<'_> {
    fn drop(&mut self) {
        self.window.layout_engine = self.engine.take();
    }
}
impl Window {
    /// Compute layout without starting a new descendant traversal stack.
    pub async fn compute_layout_async(
        &mut self,
        layout_id: LayoutId,
        available_space: Size<AvailableSpace>,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) {
        self.invalidator.debug_assert_prepaint();
        let engine = self.layout_engine.take();
        let mut restore = LayoutEngineRestore {
            window: self,
            engine,
        };
        restore
            .engine
            .as_mut()
            .unwrap()
            .compute_layout_async(layout_id, available_space, restore.window, cx, stack)
            .await;
    }
    /// Register content measurement that can perform descendant intrinsic queries.
    pub fn request_measured_layout_with_box_async<F>(
        &mut self,
        style: Style,
        mut measure: F,
    ) -> LayoutId
    where
        F: for<'a> AsyncFnMut(
                Size<Option<Pixels>>,
                Size<AvailableSpace>,
                crate::MeasuredBox,
                &'a mut Window,
                &'a mut App,
                &'a mut crate::PaintStack,
            ) -> crate::MeasuredContent
            + 'static,
    {
        self.invalidator.debug_assert_prepaint();
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        let scope = self.measure_scope();
        self.layout_engine
            .as_mut()
            .unwrap()
            .request_measured_layout_with_box_async(
                style,
                rem_size,
                scale_factor,
                async move |known,
                            available,
                            geometry,
                            window: &mut Window,
                            cx: &mut App,
                            stack: &mut crate::PaintStack| {
                    scope
                        .enter_async(window, async |window| {
                            measure(known, available, geometry, window, cx, stack).await
                        })
                        .await
                },
            )
    }
    /// Register both physical baseline sets with a lending descendant measurement.
    pub fn request_measured_layout_with_physical_baselines_async<F>(
        &mut self,
        style: Style,
        mut measure: F,
    ) -> LayoutId
    where
        F: for<'a> AsyncFnMut(
                Size<Option<Pixels>>,
                Size<AvailableSpace>,
                &'a mut Window,
                &'a mut App,
                &'a mut crate::PaintStack,
            ) -> crate::MeasuredContent
            + 'static,
    {
        self.request_measured_layout_with_box_async(
            style,
            async move |known,
                        available,
                        _,
                        window: &mut Window,
                        cx: &mut App,
                        stack: &mut crate::PaintStack| {
                measure(known, available, window, cx, stack).await
            },
        )
    }
}

impl Window {
    /// Query one intrinsic axis without nesting a native traversal stack.
    pub async fn measure_layout_size_async(
        &mut self,
        id: LayoutId,
        available: Size<AvailableSpace>,
        axis: Axis,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) -> Size<Pixels> {
        let engine = self.layout_engine.take();
        let mut restore = LayoutEngineRestore {
            window: self,
            engine,
        };
        restore
            .engine
            .as_mut()
            .expect("layout engine during measurement")
            .measure_size_async(id, available, axis, restore.window, cx, stack)
            .await
    }
}
