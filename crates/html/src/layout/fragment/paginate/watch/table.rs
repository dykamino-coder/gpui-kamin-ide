//! Запись раскладки таблиц копии: раскладка таблицы (`layout::table`) оборачивает коробку
//! таблицы, коробку сетки рядов и один элемент сетки прозрачной обёрткой (`table_part`), если
//! рекордер сборки наблюдает таблицу; после раскладки кадр таблицы (`TableFrame`) — их border
//! box и дорожки рядов родителя-сетки элемента (`Window::parent_grid_tracks`).

use super::Watched;
use crate::hooks::{Hooks, IntoHooked};
use crate::layout::fragment::flow::blocks::TableFrame;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Point, Window};
use std::collections::HashMap;
use std::rc::Rc;

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
        Some(rec) if rec.tables.contains(&key) => TableWatch {
            key,
            part,
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

impl Watched {
    /// Кадры записанных таблиц относительно `origin`.
    pub(super) fn table_frames(
        &self,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> HashMap<u64, TableFrame> {
        let mut out = HashMap::new();
        for (&key, nodes) in self.table_nodes.borrow().iter() {
            let (Some(table), Some(grid)) = (nodes.table, nodes.grid) else {
                continue;
            };
            let mut span = |lid: LayoutId| {
                let top = f32::from(window.layout_origin_unrounded(lid).y - origin.y);
                (
                    top,
                    top + f32::from(window.layout_size_unrounded(lid).height),
                )
            };
            let (table, grid) = (span(table), span(grid));
            let rows = nodes
                .item
                .and_then(|item| window.parent_grid_tracks(item))
                .map(|(at, _, rows)| {
                    let y = f32::from(at.y - origin.y);
                    rows.into_iter().map(|(a, b)| (y + a, y + b)).collect()
                })
                .unwrap_or_default();
            out.insert(key, TableFrame { table, grid, rows });
        }
        out
    }
}

struct TableWatch {
    key: u64,
    part: TablePart,
    inner: AnyElement,
    rec: Rc<Watched>,
}

impl Hooks for TableWatch {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        let mut nodes = self.rec.table_nodes.borrow_mut();
        let n = nodes.entry(self.key).or_default();
        match self.part {
            TablePart::Box => n.table = Some(lid),
            TablePart::Grid => n.grid = Some(lid),
            TablePart::Item => n.item = Some(lid),
        }
        (lid, ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
    }
}
