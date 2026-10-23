//! Разрывы по реальным flex-линиям (css-flexbox-1 §10): элементы ряда — отдельные потоки.
use super::geometry::{Pending, walk};
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
            // Строки многострочной колонки — параллельные потоки: граница
            // между элементами одной строки не разрывает соседние, общих точек
            // класса A у контейнера нет (css-break-3 §4.3; Blink копит ранние
            // разрывы отдельно на каждую колонку, `FlexColumnBreakInfo`), и
            // контейнер режется краем фрагментаинера.
            let single = flows.len() <= 1;
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
                if single {
                    // Однострочная колонка — один поток: break-before/after
                    // её элементов — разрывы класса A между соседями
                    // (css-flexbox-1 §10), их видит и балансировка.
                    out.cuts.extend(flow.cuts);
                    out.forced.extend(flow.forced);
                }
                // Avoid внутри независимой колонки обслуживает её Regrow:
                // перенос не должен сдвигать содержимое соседних колонок.
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
                let prev_items = self.items.iter().filter(|i| prev.items.contains(&i.id));
                let items = self.items.iter().filter(|i| line.items.contains(&i.id));
                let force = prev_items.clone().any(|i| i.edge(true, laid).0)
                    || items.clone().any(|i| i.edge(false, laid).0);
                // Поля, примыкающие к непринудительному разрыву, усекаются, после
                // принудительного — сохраняются (css-break-3 §5.2): продолжение
                // начинается с border box первого элемента линии, как и после
                // промежутка (row-gap) у разрыва.
                let next = if force {
                    line.start
                } else {
                    line_top(line, laid)
                };
                out.cuts.push((prev.end, next));
                if next > prev.end + COORD_EPS {
                    out.solid.push((prev.end, next + COORD_EPS));
                }
                if force {
                    out.forced.push(line.start);
                } else if prev_items.clone().any(|i| i.edge(true, laid).1)
                    || items.clone().any(|i| i.edge(false, laid).1)
                {
                    boundary_avoid[index] = true;
                    let j = (0..index).rev().find(|&j| !boundary_avoid[j]).unwrap_or(0);
                    out.avoid.push((frame.lines[j].start, line.start + 0.05));
                    out.pending.push(Pending::Avoid {
                        top: prev.start,
                        bottom: prev.end,
                        floor,
                        next: line.start,
                    });
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

/// Начало содержимого линии ряда: border box её первого элемента.
pub(super) fn line_top(line: &super::FlexLine, laid: &Laid) -> f32 {
    line.items
        .iter()
        .filter_map(|id| laid.at(*id).map(|p| p.0))
        .min_by(f32::total_cmp)
        .map_or(line.start, |top| top.max(line.start))
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
                    flex_gap: Some(self.main_gap),
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
        for b in self.kids.iter().filter_map(|i| i.inner.as_ref()).chain(
            self.grid
                .iter()
                .flat_map(|g| &g.items)
                .filter_map(|i| i.inner.as_ref()),
        ) {
            b.flex_flows(laid, out);
        }
    }
}
