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
            // Единственная колонка — последовательный поток: его точки класса A
            // и все ограничения видны контейнеру (css-flexbox-1 §10).
            // При нескольких колонках потоки независимы: внутренний перенос
            // обслуживает Regrow каждого потока (Blink `FlexColumnBreakInfo`).
            let single = flows.len() <= 1;
            for b in flows {
                let mut flow = BlockBreaks::default();
                walk(&b, floor, f32::MAX, laid, &mut flow);
                if single {
                    out.cuts.extend(flow.cuts);
                    out.forced.extend(flow.forced);
                    out.solid.extend(flow.solid);
                    out.barriers.extend(flow.barriers);
                    out.keep.extend(flow.keep);
                    out.avoid_inside.extend(flow.avoid_inside);
                    out.avoid.extend(flow.avoid);
                    out.pending.extend(flow.pending);
                } else {
                    let first = b
                        .kids
                        .first()
                        .and_then(|k| laid.at(k.id))
                        .map_or(floor, |p| p.0);
                    out.solid
                        .extend(flow.solid.into_iter().filter(|s| s.0 <= first + COORD_EPS));
                    out.barriers.extend(
                        flow.barriers
                            .into_iter()
                            .filter(|s| s.0 <= first + COORD_EPS),
                    );
                }
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
                // начинается с border box первого элемента линии.
                // Эффективный промежуток перед линией (row-gap и место
                // align-content) — пустое место контейнера: мягкий разрыв ложится
                // и в него, а непоглощённую часть гасит рост линии
                // (`suppressed_row_gaps`), не пропуск содержимого копии —
                // заданный размер контейнера сохраняется (css-gaps-1 §2; Blink
                // `UpdateOffsetAdjustmentForSuppressedRowGap`).
                let (from, next) = if force {
                    (prev.end, line.start)
                } else {
                    (line.start, line_top(line, laid))
                };
                out.cuts.push((from, next));
                if next > from + COORD_EPS {
                    out.hold((from, next + COORD_EPS));
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
                    flow.hold((top, bottom));
                } else if let Some(inner) = &item.inner {
                    walk(inner, top, bottom, laid, &mut flow);
                }
                // Первая неделимая часть элемента у начала линии переносит
                // линию целиком; элемент, выровненный ниже начала линии,
                // переносится сам (Blink сдвигает элемент, а не ряд). Остальные
                // диапазоны и forced принадлежат item; их рост считает Regrow.
                let at_start =
                    laid.margin_box(item.id).map_or(top, |m| m.0) <= line.start + COORD_EPS;
                out.solid.extend(
                    flow.solid
                        .iter()
                        .filter(|s| s.0 <= top + COORD_EPS)
                        .map(|s| if at_start { (line.start, s.1) } else { *s }),
                );
                out.barriers.extend(
                    flow.barriers
                        .iter()
                        .filter(|s| s.0 <= top + COORD_EPS)
                        .map(|s| if at_start { (line.start, s.1) } else { *s }),
                );
                out.cuts.extend(flow.cuts);
                out.keep.extend(flow.keep);
                out.avoid_inside.extend(flow.avoid_inside);
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
    /// Потоки flex: колонки и собственное содержимое элементов, включая переполнение.
    pub(crate) fn flex_flows(&self, laid: &Laid, out: &mut Vec<BlockBox>) {
        if let Some(f) = &self.flex
            && let Some(frame) = laid.flexes.get(&self.id)
        {
            if frame.column {
                out.extend(f.column_flows(frame, laid, 0.0));
            }
            for item in &f.items {
                if let Some(b) = &item.inner {
                    out.push(b.clone());
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
