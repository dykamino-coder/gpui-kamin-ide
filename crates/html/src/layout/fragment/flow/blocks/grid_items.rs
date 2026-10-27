//! Элементы сеток стопки как потоки со своим расходом (css-grid-2 §12: доступное элементу место
//! уменьшается на израсходованное им самим; Blink `PlaceGridItemsForFragmentation`
//! `unavailable_block_size`). Монолит одного элемента растягивает фрагмент сетки, а соседний
//! элемент продолжается с конца своего фрагмента, а не с конца растянутого фрагмента сетки:
//! копия следующего фрагмента сдвигает его на недоступное ему место.

use super::geometry::walk;
use super::{BlockBox, BlockBreaks, Laid};
use crate::layout::fragment::flow::FlowGeometry;
use crate::tolerance::COORD_EPS;
use std::collections::HashMap;

/// Элемент внешней сетки стопки и то, что решает его продолжение.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResumedItem {
    pub(crate) grid: u64,
    /// Последняя дорожка рядов сетки: место после неё продлевает сетку.
    pub(crate) last_track: usize,
    pub(crate) id: u64,
    pub(crate) top: f32,
    /// Конец содержимого элемента (без растяжения областью).
    pub(crate) end: f32,
    /// Монолиты и декорации элемента: их край фрагментаинера не режет.
    pub(crate) barriers: Vec<(f32, f32)>,
}

/// Фрагмент ребёнка стопки: копия, начало и конец его окна в потоке и край фрагментаинера.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ItemFrame {
    pub(crate) copy: usize,
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) edge: f32,
}

/// Как копия рисует элемент: сдвиг на недоступное ему место и обрезка по концу его
/// фрагмента — координата потока копии, отсчитанная от верха несдвинутого элемента.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ItemWindow {
    pub(crate) shift: f32,
    pub(crate) clip: Option<f32>,
}

/// Окна элементов по копиям и конец содержимого сетки с их сдвигами.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ItemResume {
    pub(crate) windows: HashMap<(usize, u64), ItemWindow>,
    /// Наибольший конец содержимого элементов каждой сетки после сдвигов.
    pub(crate) ends: HashMap<u64, (usize, f32)>,
}

impl BlockBox {
    /// Элементы сеток стопки, не вложенных в элементы других сеток.
    pub(crate) fn resumed_grid_items(&self, laid: &Laid, out: &mut Vec<ResumedItem>) {
        if let Some(g) = &self.grid {
            let tracks = laid.rows(self.id).map_or(0, |r| r.len());
            for it in &g.items {
                let Some((top, bottom)) = laid.at(it.id) else {
                    continue;
                };
                let mut flow = BlockBreaks::default();
                let end = match &it.inner {
                    Some(inner) if !it.monolith => {
                        walk(inner, top, bottom, laid, &mut flow);
                        content_end(inner, laid).unwrap_or(top)
                    }
                    _ => {
                        flow.hold((top, bottom));
                        bottom
                    }
                };
                out.push(ResumedItem {
                    grid: self.id,
                    last_track: tracks.saturating_sub(1),
                    id: it.id,
                    top,
                    end,
                    barriers: flow.finish(FlowGeometry::default(), f32::MAX).barriers,
                });
            }
            return;
        }
        for inner in self.kids.iter().filter_map(|k| k.inner.as_ref()) {
            inner.resumed_grid_items(laid, out);
        }
    }
}

impl BlockBox {
    /// Начала margin box элементов всех сеток стопки: элемент принадлежит фрагменту, где
    /// начинается его ряд, даже если отрицательное поле вынесло его коробку выше
    /// (`paginate::watch::owner`; Blink кладёт элемент в смещение его ряда).
    pub(crate) fn grid_item_starts(&self, laid: &Laid, out: &mut Vec<(u64, f32)>) {
        if let Some(g) = &self.grid {
            out.extend(
                g.items
                    .iter()
                    .filter_map(|it| laid.at(it.id).map(|p| (it.id, p.0 - it.mt))),
            );
        }
        let grid_items = self.grid.iter().flat_map(|g| &g.items).map(|i| &i.inner);
        let kids = self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
            .map(|k| &k.inner);
        for inner in grid_items.chain(kids).flatten() {
            inner.grid_item_starts(laid, out);
        }
    }
}

/// Конец содержимого коробки: её дети и строки, без растяжения самой коробки.
fn content_end(b: &BlockBox, laid: &Laid) -> Option<f32> {
    let kids = b
        .kids
        .iter()
        .map(|k| k.id)
        .chain(b.flex.iter().flat_map(|f| f.items.iter().map(|i| i.id)))
        .chain(b.grid.iter().flat_map(|g| g.items.iter().map(|i| i.id)));
    let lines = laid
        .flows
        .get(&b.id)
        .into_iter()
        .flat_map(|f| f.solid.iter().map(|s| s.1));
    kids.filter_map(|id| laid.at(id).map(|p| p.1))
        .chain(lines)
        .max_by(f32::total_cmp)
}

/// Продолжение каждого элемента по фрагментам `frames` (в порядке копий): элемент, чей
/// фрагмент кончился раньше фрагмента сетки (её растянул монолит соседа), в следующей копии
/// сдвигается на недоступное ему место и обрезается по своему концу в этой.
pub(crate) fn resume_items(items: &[ResumedItem], frames: &[ItemFrame]) -> ItemResume {
    let mut out = ItemResume::default();
    for item in items {
        let mut shift = 0.0f32;
        for f in frames {
            let (top, end) = (item.top + shift, item.end + shift);
            if top >= f.end - COORD_EPS || end <= f.start + COORD_EPS {
                continue;
            }
            // Монолит элемента, пересекающий край, кончает фрагмент элемента своим концом.
            let stop = item
                .barriers
                .iter()
                .map(|&(a, b)| (a + shift, b + shift))
                .filter(|&(a, b)| a < f.edge - COORD_EPS && b > f.edge + COORD_EPS)
                .map(|(_, b)| b)
                .fold(f.edge, f32::max)
                .min(f.end);
            let continues = end > stop + COORD_EPS && stop < f.end - COORD_EPS;
            let window = ItemWindow {
                shift,
                clip: continues.then_some(stop - item.top),
            };
            if window != ItemWindow::default() {
                out.windows.insert((f.copy, item.id), window);
            }
            if continues {
                shift += f.end - stop;
            }
        }
        let grid_end = out.ends.entry(item.grid).or_insert((item.last_track, 0.0));
        grid_end.1 = grid_end.1.max(item.end + shift);
    }
    out
}

#[cfg(test)]
mod tests;
