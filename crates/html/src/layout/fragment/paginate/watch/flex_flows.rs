//! Строковые боксы flex-элемента записываются в его собственный параллельный поток.
use super::{Watched, axis::BlockAxis};
use crate::layout::fragment::flow::blocks::BlockBox;
use crate::layout::fragment::flow::{FlowGeometry, FlowMeasurements};
use gpui::Window;
use std::collections::HashMap;
use std::rc::Rc;

impl Watched {
    pub(crate) fn rewind_flows(&self) {
        self.flow.rewind();
        for copy in self.copies.borrow().values() {
            copy.rewind_flows();
        }
        for flow in self.item_flows.values() {
            flow.rewind();
        }
    }

    pub(crate) fn consume_flex_items(
        &self,
        values: HashMap<u64, (f32, bool)>,
        window: &mut Window,
    ) {
        for (&id, &(amount, _)) in &values {
            if let Some(lid) = self.seen(id) {
                window.set_flex_item_fragmentation_consumed(lid, amount);
            }
        }
        *self.flex_consumed.borrow_mut() = values;
    }

    pub(crate) fn clear_consumed(&self, window: &mut Window) {
        let ids: Vec<_> = self.flex_consumed.borrow().keys().copied().collect();
        for id in ids {
            if let Some(lid) = self.seen(id) {
                window.set_flex_item_fragmentation_consumed(lid, 0.0);
            }
        }
        self.flex_consumed.borrow_mut().clear();
    }

    pub(crate) fn resume_content(
        &self,
        laid: &crate::layout::fragment::flow::blocks::Laid,
        from: f32,
    ) {
        for (&id, lines) in &laid.content {
            if let Some(flow) = self.item_flows.get(&id) {
                flow.resume_lines(lines, from);
            }
        }
    }

    pub(super) fn flex_item_flows(blocks: &BlockBox) -> Rc<HashMap<u64, Rc<FlowMeasurements>>> {
        let mut flexes = Vec::new();
        blocks.flexes(&mut flexes);
        Rc::new(
            flexes
                .into_iter()
                .flat_map(|(_, items)| items)
                .map(|id| (id, Rc::new(FlowMeasurements::for_lines())))
                .collect(),
        )
    }

    pub(crate) fn for_flex_item(&self, id: u64) -> Option<Rc<Self>> {
        let flow = self.item_flows.get(&id)?.clone();
        let mut scope = self.nested();
        Rc::get_mut(&mut scope)?.flow = flow;
        Some(scope)
    }

    pub(super) fn item_geometry(
        &self,
        projection: BlockAxis,
        window: &mut Window,
    ) -> HashMap<u64, FlowGeometry> {
        self.item_flows
            .iter()
            .map(|(&id, flow)| {
                (
                    id,
                    flow.at(projection.origin, projection.size, projection.axis, window),
                )
            })
            .collect()
    }
}
