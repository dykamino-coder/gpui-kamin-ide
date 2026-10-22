//! Разрывы по реальным flex-линиям (css-flexbox-1 §10): элементы ряда — отдельные потоки.
use super::geometry::walk;
use super::{BlockBox, BlockBreaks, FlexBox, Laid};
use crate::tolerance::COORD_EPS;

impl FlexBox {
    pub(super) fn breaks(
        &self,
        id: u64,
        floor: f32,
        laid: &Laid,
        out: &mut BlockBreaks,
    ) -> Option<f32> {
        let frame = laid.flexes.get(&id)?;
        if frame.column {
            let flows = self.column_flows(frame, laid, floor);
            // Колонки flex независимы: наружу выходит только первая неделимая
            // часть. Внутренние forced/avoid обслуживает Regrow каждого потока.
            for b in flows {
                let mut flow = BlockBreaks::default();
                walk(&b, floor, f32::MAX, laid, &mut flow);
                let first = b
                    .kids
                    .first()
                    .and_then(|k| laid.at(k.id))
                    .map_or(floor, |p| p.0);
                out.solid
                    .extend(flow.solid.into_iter().filter(|s| s.0 <= first + COORD_EPS));
                out.cuts.extend(flow.cuts);
                out.keep.extend(flow.keep);
            }
            return self
                .items
                .iter()
                .filter_map(|i| laid.at(i.id).map(|p| p.1))
                .max_by(f32::total_cmp);
        }
        let mut boundary_avoid = vec![false; frame.lines.len()];
        for (index, line) in frame.lines.iter().enumerate() {
            if index > 0 {
                let prev = &frame.lines[index - 1];
                out.cuts.push((prev.end, line.start));
                if line.start > prev.end + COORD_EPS {
                    out.solid.push((prev.end, line.start + COORD_EPS));
                }
                let prev_items = self.items.iter().filter(|i| prev.items.contains(&i.id));
                let items = self.items.iter().filter(|i| line.items.contains(&i.id));
                let force = prev_items.clone().any(|i| i.edge(true, laid).0)
                    || items.clone().any(|i| i.edge(false, laid).0);
                if force {
                    out.forced.push(line.start);
                } else if prev_items.clone().any(|i| i.edge(true, laid).1)
                    || items.clone().any(|i| i.edge(false, laid).1)
                {
                    boundary_avoid[index] = true;
                    let j = (0..index).rev().find(|&j| !boundary_avoid[j]).unwrap_or(0);
                    out.avoid
                        .push((frame.lines[j].start, line.start + COORD_EPS));
                }
            }
            for item in self.items.iter().filter(|i| line.items.contains(&i.id)) {
                let Some((top, bottom)) = laid.at(item.id) else {
                    continue;
                };
                let mut flow = BlockBreaks::default();
                if item.monolith {
                    flow.solid.push((top, bottom));
                } else if let Some(inner) = &item.inner {
                    walk(inner, top, bottom, laid, &mut flow);
                }
                // Первая неделимая часть переносит линию целиком. Остальные
                // диапазоны и forced принадлежат item; их рост считает Regrow.
                out.solid.extend(
                    flow.solid
                        .iter()
                        .filter(|s| s.0 <= top + COORD_EPS)
                        .map(|s| (line.start, s.1)),
                );
                out.cuts.extend(flow.cuts);
                out.keep.extend(flow.keep);
            }
        }
        frame.lines.last().map(|l| l.end)
    }
}

impl FlexBox {
    fn column_flows(&self, frame: &super::FlexFrame, laid: &Laid, floor: f32) -> Vec<BlockBox> {
        frame
            .lines
            .iter()
            .map(|line| {
                let mut kids: Vec<_> = line
                    .items
                    .iter()
                    .filter_map(|id| self.items.iter().find(|i| i.id == *id))
                    .cloned()
                    .collect();
                kids.sort_by(|a, b| {
                    laid.at(a.id)
                        .map(|p| p.0)
                        .unwrap_or(floor)
                        .total_cmp(&laid.at(b.id).map(|p| p.0).unwrap_or(floor))
                });
                for kid in &mut kids {
                    kid.adjoins = true;
                }
                BlockBox {
                    kids,
                    ..Default::default()
                }
            })
            .collect()
    }
}

impl BlockBox {
    /// Независимые потоки flex: элементы ряда либо последовательность элементов колонки.
    pub(crate) fn flex_flows(&self, laid: &Laid, out: &mut Vec<BlockBox>) {
        if let Some(f) = &self.flex
            && let Some(frame) = laid.flexes.get(&self.id)
        {
            if frame.column {
                out.extend(f.column_flows(frame, laid, 0.0));
            } else {
                for item in &f.items {
                    if let Some(b) = &item.inner {
                        out.push(b.clone());
                    }
                }
            }
            for item in &f.items {
                if let Some(b) = &item.inner {
                    b.flex_flows(laid, out);
                }
            }
        }
        for b in self
            .kids
            .iter()
            .filter_map(|i| i.inner.as_ref())
            .chain(
                self.grid
                    .iter()
                    .flat_map(|g| &g.items)
                    .filter_map(|i| i.inner.as_ref()),
            )
            .chain(self.table.iter().flat_map(|t| t.flows()))
        {
            b.flex_flows(laid, out);
        }
    }
}
