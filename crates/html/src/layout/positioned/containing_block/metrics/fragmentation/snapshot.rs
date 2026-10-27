//! Завершённая исходная геометрия OOF-потоков без вклада в normal flow.

use super::*;

impl Recorder {
    pub(crate) fn fragment_snapshot(&self, snapshot: &mut Snapshot, window: &mut Window) {
        let axis = self.flows.axis;
        snapshot.axis = Some(axis);
        let mut contexts = Vec::with_capacity(self.flows.flows.len());
        let mut roots: Vec<u64> = Vec::with_capacity(self.flows.flows.len());
        let outside = self.outside(snapshot);
        // A continued root cannot hold a box laid out by a block outside the
        // source: that box keeps its static place in the root's legacy copies,
        // so the whole family stays there.
        let mut held = std::collections::HashSet::new();
        let mut walk: Vec<u64> = Vec::with_capacity(self.flows.flows.len());
        for (index, flow) in self.flows.flows.iter().enumerate() {
            let root = flow.parent.map_or(flow.id, |parent| walk[parent]);
            walk.push(root);
            if outside[index] && flow.parent.is_some_and(|parent| !outside[parent]) {
                held.insert(root);
            }
        }
        for (index, flow) in self.flows.flows.iter().enumerate() {
            let (parent_repeat, parent_fixed) = flow
                .parent
                .map_or((false, false), |parent| contexts[parent]);
            let repeated = parent_repeat || flow.policy.get() == FixedPolicy::RepeatPages;
            let fixed_content = parent_fixed || flow.fixed.get();
            contexts.push((repeated, fixed_content));
            let root = flow.parent.map_or(flow.id, |parent| roots[parent]);
            roots.push(root);
            let metrics = snapshot.get(flow.id).copied();
            let escapes = outside[index] || held.contains(&root);
            let Some(metrics) = metrics.filter(|_| !escapes) else {
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
                root,
            });
        }
    }

    /// Flows laid out by a block outside the multicol, not fragmented by its
    /// columns (css-multicol-1 §2, css-break-3 §2), and anchor-positioned flows,
    /// placed by anchors the source does not lay out; descendants follow them.
    fn outside(&self, snapshot: &Snapshot) -> Vec<bool> {
        let mut outside: Vec<bool> = Vec::with_capacity(self.flows.flows.len());
        for flow in &self.flows.flows {
            let escapes = flow.parent.is_some_and(|parent| outside[parent])
                || flow.anchored
                || snapshot.get(flow.id).is_some_and(|m| {
                    snapshot.host != 0
                        && m.cb_id != snapshot.host
                        && !self.flows.families.contains_key(&m.cb_id)
                });
            outside.push(escapes);
        }
        outside
    }
}
