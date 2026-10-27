//! Реальные flex-линии, нормальные позиции и ограничения роста между фрагментами.
use super::Watched;
use std::rc::Rc;

impl Watched {
    /// Flex-контейнер наблюдаемой стопки: его раскладка — в контексте
    /// фрагментации (элементы колонки охватывают своё содержимое).
    pub(crate) fn flex_container(&self, id: u64) -> bool {
        self.flex_ids_of().contains(&id)
    }

    pub(super) fn flex_ids_of(&self) -> &std::collections::HashSet<u64> {
        self.flex_ids.get_or_init(|| {
            let mut flexes = Vec::new();
            if let Some(b) = &self.blocks {
                b.flexes(&mut flexes);
            }
            flexes.into_iter().map(|(id, _)| id).collect()
        })
    }

    /// Сдвиг элемента колонки: положительный — перенос за разрыв,
    /// отрицательный — погашенный у разрыва промежуток.
    pub(crate) fn grow_flex_item(&self, id: u64, growth: f32) -> bool {
        if !growth.is_finite() || growth.abs() <= 0.0 {
            return false;
        }
        *self.flex_offsets.borrow_mut().entry(id).or_default() += growth;
        true
    }

    /// Разбиение на линии предшествует фрагментации (css-flexbox-1 §10): рост
    /// содержимого элемента колонки разрывом не переносит его в другую линию.
    /// Линии берутся из раскладки до первого роста; номер — native-индекс
    /// final metadata (`FlexLine::index`), а не порядок вдоль блочной оси.
    /// Колонка с переносом, пока занимающая одну линию, тоже закрепляется:
    /// рост элемента разрывом не открывает новую линию.
    pub(crate) fn pin_flex_lines(&self, laid: &crate::layout::fragment::flow::blocks::Laid) {
        let mut pins = self.flex_lines.borrow_mut();
        for frame in laid.flexes.values().filter(|f| f.column) {
            if frame
                .lines
                .iter()
                .flat_map(|l| &l.items)
                .any(|id| pins.contains_key(id))
            {
                continue;
            }
            for line in &frame.lines {
                for &id in &line.items {
                    pins.insert(id, line.index);
                }
            }
        }
    }

    pub(crate) fn grow_flex_line(&self, id: u64, line: usize, growth: f32) -> bool {
        if !growth.is_finite() || growth <= 0.0 {
            return false;
        }
        let mut all = self.flex_growth.borrow_mut();
        let lines = all.entry(id).or_default();
        lines.resize(lines.len().max(line + 1), 0.0);
        lines[line] += growth;
        true
    }

    /// Запасная копия применяет те же ограничения, но не пишет геометрию главной.
    pub(crate) fn copy_recorder(&self, part: usize) -> Rc<Self> {
        self.copies
            .borrow_mut()
            .entry(part)
            .or_insert_with(|| self.constraints_only())
            .clone()
    }

    /// Другая ширина требует своих распорок: адрес содержимого общий, геометрия новая.
    pub(crate) fn continuation_recorder(&self, part: usize) -> Rc<Self> {
        if let Some(rec) = self.copies.borrow().get(&part).filter(|r| r.independent) {
            return rec.clone();
        }
        let rec = Rc::new(Self {
            ids: self.ids.clone(),
            blocks: self.blocks.clone(),
            monolithic_ids: self.monolithic_ids_of().clone().into(),
            tables: self.tables.clone(),
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasure::for_lines()),
            item_flows: Rc::new(
                self.item_flows
                    .keys()
                    .map(|&id| {
                        (
                            id,
                            Rc::new(crate::layout::fragment::flow::FlowMeasure::for_lines()),
                        )
                    })
                    .collect(),
            ),
            independent: true,
            axis: self.axis,
            ..Self::default()
        });
        self.copies.borrow_mut().insert(part, rec.clone());
        rec
    }

    pub(crate) fn constraints_only(&self) -> Rc<Self> {
        Rc::new(Self {
            ids: self.ids.clone(),
            blocks: self.blocks.clone(),
            monolithic_ids: self.monolithic_ids_of().clone().into(),
            tables: self.tables.clone(),
            flow: Rc::new(self.flow.constraints_only()),
            flex_growth: self.flex_growth.clone(),
            grid_growth: self.grid_growth.clone(),
            flex_offsets: self.flex_offsets.clone(),
            axis: self.axis,
            item_flows: Rc::new(
                self.item_flows
                    .iter()
                    .map(|(&id, flow)| (id, Rc::new(flow.constraints_only())))
                    .collect(),
            ),
            flex_lines: self.flex_lines.clone(),
            paint_owners: self.paint_owners.clone(),
            paint_outer: self.paint_outer.clone(),
            ..Self::default()
        })
    }
}
