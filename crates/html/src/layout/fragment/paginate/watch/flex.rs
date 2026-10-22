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
}

impl Watched {
    pub(crate) fn grow_flex_item(&self, id: u64, growth: f32) -> bool {
        if !growth.is_finite() || growth <= 0.0 {
            return false;
        }
        *self.flex_offsets.borrow_mut().entry(id).or_default() += growth;
        true
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
    pub(crate) fn constraints_only(&self) -> Rc<Self> {
        Rc::new(Self {
            ids: self.ids.clone(),
            flex_growth: self.flex_growth.clone(),
            flex_offsets: self.flex_offsets.clone(),
            item_flows: Rc::new(
                self.item_flows
                    .iter()
                    .map(|(&id, flow)| (id, Rc::new(flow.constraints_only())))
                    .collect(),
            ),
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
        }
    }
}
