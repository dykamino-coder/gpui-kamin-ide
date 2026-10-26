//! Создание рекордера с потоком BFC до разделения его владения.
use super::*;
use crate::layout::fragment::flow::FlowMeasure;

impl Watched {
    pub(crate) fn new(ids: &[u64]) -> Rc<Self> {
        Rc::new(Watched {
            ids: Rc::new(ids.iter().copied().collect()),
            seen: Rc::default(),
            flow: Rc::default(),
            ..Self::default()
        })
    }

    /// Ось копии задаёт направление native flex growth и consumed-space clipping.
    pub(crate) fn for_blocks_axis(blocks: BlockBox, axis: StackAxis, lines: bool) -> Rc<Self> {
        let flow = if lines {
            Rc::new(crate::layout::fragment::flow::FlowMeasure::for_lines())
        } else {
            Rc::default()
        };
        Self::for_blocks_flow(blocks, axis, flow)
    }

    /// Готовый поток принадлежит BFC до публикации рекордера.
    pub(crate) fn for_blocks_flow(
        blocks: BlockBox,
        axis: StackAxis,
        flow: Rc<FlowMeasure>,
    ) -> Rc<Self> {
        Rc::new(Watched {
            // With the growth struts before them (`STRUT_ID`).
            ids: Rc::new(
                blocks
                    .ids()
                    .into_iter()
                    .flat_map(|id| [id, id ^ crate::layout::fragment::flow::blocks::STRUT_ID])
                    .collect(),
            ),
            seen: Rc::default(),
            tables: Rc::new(blocks.table_keys().into_iter().collect()),
            flow,
            item_flows: Self::flex_item_flows(&blocks),
            blocks: Some(blocks),
            axis: Some(axis),
            ..Self::default()
        })
    }

    pub(crate) fn for_lines() -> Rc<Self> {
        Self::with_flow(Rc::new(FlowMeasure::for_lines()))
    }

    pub(crate) fn with_flow(flow: Rc<FlowMeasure>) -> Rc<Self> {
        Rc::new(Self {
            flow,
            ..Self::default()
        })
    }
}
