//! Запись раскладки таблиц копии: раскладка таблицы (`layout::table`) оборачивает коробку
//! таблицы, коробку сетки рядов и один элемент сетки прозрачной обёрткой (`table_part`), если
//! рекордер сборки наблюдает таблицу; после раскладки кадр таблицы (`TableFrame`) — их border
//! box и дорожки рядов родителя-сетки элемента (`Window::parent_grid_tracks`).

use super::{Slot, Watch, Watched, axis::BlockAxis};
use crate::hooks::IntoHooked;
use crate::layout::fragment::flow::blocks::TableFrame;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, LayoutId, Window};
use std::collections::HashMap;

/// Часть таблицы, чей узел раскладки нужен её кадру.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TablePart {
    /// Коробка таблицы (рамка и отбивка, без подписей).
    Box,
    /// Коробка сетки рядов.
    Grid,
    /// Элемент сетки: по нему читаются её дорожки.
    Item,
}

/// Узлы частей таблицы последней раскладки.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TableNodes {
    table: Option<LayoutId>,
    grid: Option<LayoutId>,
    item: Option<LayoutId>,
}

/// Обернуть часть `part` таблицы с ключом `key`, если рекордер сборки её
/// наблюдает (иначе — как есть).
pub(crate) fn table_part(
    key: u64,
    part: TablePart,
    el: AnyElement,
    opts: &RenderOpts,
) -> AnyElement {
    match &opts.frag.watch {
        Some(rec) if rec.tables.contains(&key) => Watch {
            slot: Slot::Table(key, part),
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

impl Watched {
    /// Узел части `part` таблицы `key` последней раскладки.
    pub(super) fn record_part(&self, key: u64, part: TablePart, lid: LayoutId) {
        let mut nodes = self.table_nodes.borrow_mut();
        let n = nodes.entry(key).or_default();
        match part {
            TablePart::Box => n.table = Some(lid),
            TablePart::Grid => n.grid = Some(lid),
            TablePart::Item => n.item = Some(lid),
        }
    }

    /// Кадры записанных таблиц относительно `origin`.
    pub(super) fn table_frames(
        &self,
        axis: BlockAxis,
        window: &mut Window,
    ) -> HashMap<u64, TableFrame> {
        let mut out = HashMap::new();
        for (&key, nodes) in self.table_nodes.borrow().iter() {
            let (Some(table), Some(grid)) = (nodes.table, nodes.grid) else {
                continue;
            };
            let mut span = |lid: LayoutId| {
                axis.span(
                    window.layout_origin_unrounded(lid),
                    window.layout_size_unrounded(lid),
                )
            };
            let (table, grid) = (span(table), span(grid));
            let rows = nodes
                .item
                .and_then(|item| window.parent_grid_tracks(item))
                .map(|(at, columns, rows)| axis.tracks(at, columns, rows, true))
                .unwrap_or_default();
            out.insert(key, TableFrame { table, grid, rows });
        }
        out
    }
}
