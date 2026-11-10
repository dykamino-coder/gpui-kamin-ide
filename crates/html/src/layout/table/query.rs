//! Запросы виртуальных табличных коробок читают готовые дорожки канонической сетки.

use super::context::TableGeom;
use crate::layout::{build_contracts::RefBox, geometry::Collector};
use gpui::{Bounds, LayoutId, LayoutObserver, Pixels, Window, point, px, size};
use std::collections::HashMap;

mod parts;
use parts::Part;

pub(super) struct TableQuery {
    parts: Vec<Part>,
    refs: HashMap<u64, RefBox>,
    vertical: bool,
    reverse_rows: bool,
    reverse_columns: bool,
    layout: Option<LayoutId>,
}

impl TableQuery {
    pub(super) fn new(
        geom: &TableGeom<'_>,
        refs: &HashMap<u64, RefBox>,
        opts: &crate::layout::options::RenderOpts,
    ) -> Self {
        let parts = parts::collect(geom, opts);
        let wm = geom.e.style.used().wm();
        Self {
            parts,
            refs: refs.clone(),
            vertical: wm.vertical(),
            reverse_rows: wm.vertical() && wm.block_rl(),
            reverse_columns: if wm.vertical() {
                (wm.rtl() && !wm.upright()) != wm.sideways_lr()
            } else {
                wm.rtl()
            },
            layout: None,
        }
    }
}

impl LayoutObserver for TableQuery {
    fn request_layout(&mut self, id: LayoutId, _: &Window) {
        self.layout = Some(id);
    }

    fn prepaint(&mut self, window: &mut Window) -> bool {
        let Some(id) = self.layout else { return true };
        let Some((origin, x, y)) = window.grid_tracks(id) else {
            return true;
        };
        let (columns, rows) = if self.vertical { (&y, &x) } else { (&x, &y) };
        let extent =
            |tracks: &[(f32, f32)]| tracks.first().zip(tracks.last()).map(|(a, b)| (a.0, b.1));
        let grid_size = window.layout_size_unrounded(id);
        let (inline, block) = if self.vertical {
            (grid_size.height, grid_size.width)
        } else {
            (grid_size.width, grid_size.height)
        };
        let all_columns = extent(columns).unwrap_or((0., inline.into()));
        let all_rows = extent(rows).unwrap_or((0., block.into()));
        let physical = |column: (f32, f32), row: (f32, f32), shift: (f32, f32)| {
            let (x, y) = if self.vertical {
                (row, column)
            } else {
                (column, row)
            };
            Bounds::new(
                origin + point(px(x.0 + shift.0), px(y.0 + shift.1)),
                size(px((x.1 - x.0).max(0.)), px((y.1 - y.0).max(0.))),
            )
        };
        let mut boxes = HashMap::<u64, Bounds<Pixels>>::new();
        for part in &self.parts {
            if let Some(boundary) = part.empty_row {
                let boundary = if self.reverse_rows {
                    rows.len().saturating_sub(boundary)
                } else {
                    boundary
                };
                let edge = rows.get(boundary).map_or(all_rows.1, |r| r.0);
                boxes.insert(part.node, physical(all_columns, (edge, edge), part.shift));
            } else if let Some(i) = part.row {
                let i = if self.reverse_rows {
                    rows.len().saturating_sub(i + 1)
                } else {
                    i
                };
                let empty = (all_rows.0, all_rows.0);
                if let Some(row) = rows.get(i).or_else(|| rows.is_empty().then_some(&empty)) {
                    let bounds = physical(all_columns, *row, part.shift);
                    boxes.insert(part.node, bounds);
                    if let Some(group) = part.group {
                        let bounds = physical(all_columns, *row, part.group_shift);
                        boxes
                            .entry(group)
                            .and_modify(|b| *b = b.union(&bounds))
                            .or_insert(bounds);
                    }
                }
            } else if !part.columns.is_empty() {
                let band = part
                    .columns
                    .iter()
                    .filter_map(|i| {
                        let i = if self.reverse_columns {
                            columns.len().saturating_sub(*i + 1)
                        } else {
                            *i
                        };
                        columns.get(i).copied()
                    })
                    .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
                if let Some(band) = band {
                    boxes.insert(part.node, physical(band, all_rows, (0., 0.)));
                }
            }
        }
        for (node, reference) in &self.refs {
            if let Some(bounds) = boxes.get(node) {
                reference.set(Some(*bounds));
            }
        }
        for part in &self.parts {
            let Some(bounds) = boxes.get(&part.node).copied() else {
                continue;
            };
            let record = |window: &mut Window| {
                let Some(style) = Collector::source_style(window, part.node) else {
                    return;
                };
                super::query_projection::enter(&style, bounds, false, window, |window| {
                    Collector::record(window, part.node, [bounds; 3], part.font)
                });
            };
            if let Some(group) = part
                .group
                .and_then(|node| boxes.get(&node).map(|bounds| (node, *bounds)))
            {
                if let Some(style) = Collector::source_style(window, group.0) {
                    super::query_projection::enter(&style, group.1, true, window, record);
                }
            } else if part.columns.is_empty() {
                record(window);
            } else {
                Collector::record(window, part.node, [bounds; 3], part.font);
            }
        }
        true
    }
}
