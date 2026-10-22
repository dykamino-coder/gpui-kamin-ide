//! Строковые боксы flex-элемента записываются в его собственный параллельный поток.
use super::Watched;
use crate::layout::fragment::flow::blocks::BlockBox;
use crate::layout::fragment::flow::{FlowGeometry, FlowMeasurements};
use crate::layout::fragment::types::StackAxis;
use gpui::{Pixels, Point, Window, px, size};
use std::collections::HashMap;
use std::rc::Rc;

impl Watched {
    pub(crate) fn rewind_flows(&self) {
        for flow in self.item_flows.values() {
            flow.rewind();
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
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> HashMap<u64, FlowGeometry> {
        self.item_flows
            .iter()
            .map(|(&id, flow)| {
                (
                    id,
                    flow.at(
                        origin,
                        size(px(0.0), px(0.0)),
                        StackAxis::Horizontal,
                        window,
                    ),
                )
            })
            .collect()
    }
}
