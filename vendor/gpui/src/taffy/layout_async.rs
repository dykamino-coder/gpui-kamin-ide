//! Раскладка окна продолжает общий явный стек через измерения листьев.
use super::*;
impl TaffyLayoutEngine {
    pub async fn compute_layout_async(
        &mut self,
        id: LayoutId,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) {
        // Leaving this here until we have a better instrumentation approach.
        // println!("Laying out {} children", self.count_all_children(id)?);
        // println!("Max layout depth: {}", self.max_depth(0, id)?);

        // Output the edges (branches) of the tree in Mermaid format for visualization.
        // println!("Edges:");
        // for (a, b) in self.get_edges(id)? {
        //     println!("N{} --> N{}", u64::from(a), u64::from(b));
        // }
        //

        crate::frame_perf::LAYOUT_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if !self.computed_layouts.insert(id) {
            let stack = &mut self.layout_bounds_scratch_space;
            stack.push(id);
            while let Some(id) = stack.pop() {
                self.absolute_layout_bounds.remove(&id);
                self.absolute_unrounded.remove(&id);
                stack.extend(
                    self.taffy
                        .children(id.into())
                        .expect(EXPECT_MESSAGE)
                        .into_iter()
                        .map(LayoutId::from),
                );
            }
        }

        let scale_factor = window.scale_factor();

        let transform = |v: AvailableSpace| match v {
            AvailableSpace::Definite(pixels) => {
                AvailableSpace::Definite(Pixels(pixels.0 * scale_factor))
            }
            AvailableSpace::MinContent => AvailableSpace::MinContent,
            AvailableSpace::MaxContent => AvailableSpace::MaxContent,
        };
        self.begin_geometry(id);
        let logical_space = available_space;
        let available_space = size(
            transform(available_space.width),
            transform(available_space.height),
        );

        let _taffy_started = std::time::Instant::now();
        self.taffy
            .compute_layout_with_measure_async(
                id.into(),
                available_space.into(),
                taffy::tree::AsyncMeasure(
                    async |inputs: taffy::LayoutInput,
                           _id: taffy::NodeId,
                           node_context: Option<&mut crate::taffy::NodeContext>,
                           style: &taffy::Style,
                           stack: &mut crate::PaintStack| {
                        measurement::leaf_async(
                            inputs,
                            node_context,
                            style,
                            scale_factor,
                            window,
                            cx,
                            stack,
                        )
                        .await
                    },
                ),
                stack,
            )
            .await
            .expect(EXPECT_MESSAGE);
        self.finish_geometry(id, logical_space, scale_factor);
        crate::frame_perf::TAFFY_US.fetch_add(
            _taffy_started.elapsed().as_micros() as u32,
            std::sync::atomic::Ordering::Relaxed,
        );
    }
}
