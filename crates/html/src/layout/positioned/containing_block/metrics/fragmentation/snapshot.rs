//! Завершённая исходная геометрия OOF-потоков без вклада в normal flow.

use super::*;

impl Recorder {
    pub(crate) fn fragment_snapshot(&self, snapshot: &mut Snapshot, window: &mut Window) {
        let axis = self.flows.axis;
        let mut contexts = Vec::with_capacity(self.flows.flows.len());
        for flow in &self.flows.flows {
            let (parent_repeat, parent_fixed) = flow
                .parent
                .map_or((false, false), |parent| contexts[parent]);
            let repeated = parent_repeat || flow.policy.get() == FixedPolicy::RepeatPages;
            let fixed_content = parent_fixed || flow.fixed.get();
            contexts.push((repeated, fixed_content));
            let Some(&metrics) = snapshot.get(flow.id) else {
                continue;
            };
            let Some(owner_rect) = self.normal(flow.owner, window) else {
                continue;
            };
            let rect = metrics.normal_rect();
            let h = f32::from(if axis.is_vertical() {
                rect.size.width
            } else {
                rect.size.height
            });
            let geometry = flow.watch.flow.at(rect.origin, rect.size, axis, window);
            let blocks = flow.watch.blocks().expect("OOF owns a block recorder");
            let laid = flow
                .watch
                .laid_axis(rect.origin, rect.size, axis, window)
                .with_flow(&geometry);
            let over = crate::layout::fragment::flow::flow_end(blocks, &laid).unwrap_or(h);
            let breaks = flow
                .watch
                .breaks(rect.origin, rect.size, over, axis, window);
            snapshot.flows.push(Source {
                id: flow.id,
                owner: flow.owner,
                owner_rect,
                metrics,
                kid: Kid {
                    h,
                    over,
                    monolith: flow.monolith || blocks.inside_avoid,
                    cuts: breaks.cuts,
                    forced: breaks.forced,
                    solid: breaks.solid,
                    avoid: breaks.avoid,
                    keep: breaks.keep,
                    clone_dec: flow.clone_dec.then_some((blocks.head, blocks.tail)),
                    par: crate::layout::fragment::types::Par {
                        avoid_only: !flow.monolith && blocks.inside_avoid,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                inline: Geometry {
                    floats: geometry.float_contexts,
                    paragraphs: geometry.inline,
                    barriers: breaks.barriers,
                    keep: breaks.avoid_inside,
                },
                policy: if repeated {
                    FixedPolicy::RepeatPages
                } else {
                    FixedPolicy::Fragment
                },
                fixed: flow.fixed.get(),
                inside_fixed: parent_fixed,
                static_block: flow.static_block.get(),
            });
        }
    }
}
