//! Реальные flex-линии, нормальные позиции и ограничения роста между фрагментами.
use super::Watched;
use crate::layout::fragment::flow::blocks::{FlexFrame, FlexLine};
use gpui::{Pixels, Point, Window};
use std::collections::HashMap;
use std::rc::Rc;

pub(super) struct FlexFrames {
    pub(super) flexes: HashMap<u64, FlexFrame>,
    pub(super) shifts: HashMap<u64, f32>,
    pub(super) table_shifts: HashMap<u64, f32>,
    pub(super) margins: HashMap<u64, (f32, f32)>,
}

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
    /// Линии берутся из раскладки до первого роста.
    pub(crate) fn pin_flex_lines(&self, laid: &crate::layout::fragment::flow::blocks::Laid) {
        let mut pins = self.flex_lines.borrow_mut();
        for frame in laid
            .flexes
            .values()
            .filter(|f| f.column && f.lines.len() > 1)
        {
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
            tables: self.tables.clone(),
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
            item_flows: Rc::new(
                self.item_flows
                    .keys()
                    .map(|&id| {
                        (
                            id,
                            Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
                        )
                    })
                    .collect(),
            ),
            independent: true,
            ..Self::default()
        });
        self.copies.borrow_mut().insert(part, rec.clone());
        rec
    }

    pub(crate) fn constraints_only(&self) -> Rc<Self> {
        Rc::new(Self {
            ids: self.ids.clone(),
            blocks: self.blocks.clone(),
            tables: self.tables.clone(),
            flow: Rc::new(self.flow.constraints_only()),
            flex_growth: self.flex_growth.clone(),
            flex_offsets: self.flex_offsets.clone(),
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

    pub(super) fn flex_frames(
        &self,
        origin: Point<Pixels>,
        window: &mut Window,
        boxes: &mut HashMap<u64, (f32, f32)>,
    ) -> FlexFrames {
        let mut containers = Vec::new();
        if let Some(b) = &self.blocks {
            b.flexes(&mut containers);
        }
        let seen = self.seen.borrow();
        let mut out = HashMap::new();
        let mut shifts = HashMap::<u64, f32>::new();
        let mut tables = HashMap::<u64, f32>::new();
        let mut margins = HashMap::new();
        for (id, items) in containers {
            let Some(lid) = items.iter().find_map(|id| seen.get(id)).copied() else {
                continue;
            };
            let Some((o, flex)) = window.parent_flex_layout(lid) else {
                continue;
            };
            let off = f32::from(o.y - origin.y) + shifts.get(&id).copied().unwrap_or(0.0);
            let mut lines = Vec::new();
            for (index, line) in flex.lines.into_iter().enumerate() {
                let mut members = Vec::new();
                let mut top = f32::MAX;
                let mut bottom = f32::MIN;
                for item in line.items {
                    let Some(&id) = items.iter().find(|id| seen.get(id) == Some(&item.id)) else {
                        continue;
                    };
                    let (start, size) = if flex.is_column {
                        (item.main_offset, item.main_size)
                    } else {
                        (item.cross_offset, item.cross_size)
                    };
                    let normal = off + start;
                    let delta = boxes.get(&id).map_or(0.0, |p| normal - p.0);
                    if let Some(blocks) = &self.blocks {
                        blocks.shift_flex_item(id, delta, boxes, &mut shifts, &mut tables);
                    }
                    boxes.insert(id, (normal, normal + size));
                    margins.insert(id, (item.margin_top, item.margin_bottom));
                    top = top.min(off + start);
                    bottom = bottom.max(off + start + size);
                    members.push(id);
                }
                if members.is_empty() {
                    continue;
                }
                if !flex.is_column {
                    top = off + line.cross_offset;
                    bottom = top + line.cross_size;
                }
                lines.push(FlexLine {
                    index,
                    start: top,
                    end: bottom,
                    items: members,
                });
            }
            if !flex.is_column {
                lines.sort_by(|a, b| a.start.total_cmp(&b.start));
            }
            out.insert(
                id,
                FlexFrame {
                    column: flex.is_column,
                    lines,
                },
            );
        }
        FlexFrames {
            flexes: out,
            shifts,
            table_shifts: tables,
            margins,
        }
    }
}
