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
            blocks: Some(Rc::new(blocks)),
            axis: Some(axis),
            ..Self::default()
        })
    }

    /// Проба разделяет неизменяемую модель, но имеет свои узлы и распорки.
    pub(crate) fn fresh_blocks(&self) -> Rc<Self> {
        let blocks = self.blocks.as_ref().expect("regrow records blocks");
        Rc::new(Self {
            ids: self.ids.clone(),
            tables: self.tables.clone(),
            blocks: Some(blocks.clone()),
            axis: self.axis,
            flow: if self.flow.lines {
                Rc::new(FlowMeasure::for_lines())
            } else {
                Rc::default()
            },
            item_flows: Self::flex_item_flows(blocks),
            ..Self::default()
        })
    }

    /// Наблюдать и абсолютные коробки копии с содержащим блоком внутри неё.
    pub(crate) fn with_positioned(self: Rc<Self>, ids: Vec<u64>) -> Rc<Self> {
        if !ids.is_empty() {
            // Рекордер только что создан для этой копии: список задаётся один раз.
            let _ = self.positioned.set(ids);
        }
        self
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_shares_block_model_but_not_mutable_constraints() {
        let source = Watched::for_blocks_axis(BlockBox::default(), StackAxis::Horizontal, true);
        assert!(source.grow_grid_row(3, 0, 20.));
        let trial = source.fresh_blocks();
        assert!(Rc::ptr_eq(
            source.blocks.as_ref().unwrap(),
            trial.blocks.as_ref().unwrap()
        ));
        assert!(Rc::ptr_eq(&source.ids, &trial.ids));
        assert!(!Rc::ptr_eq(&source.flow, &trial.flow));
        assert!(trial.grid_growth.borrow().is_empty());
        assert!(trial.grow_grid_row(3, 0, 10.));
        assert_eq!(source.grid_growth.borrow()[&3], [20.]);
        assert_eq!(trial.grid_growth.borrow()[&3], [10.]);
    }
}
