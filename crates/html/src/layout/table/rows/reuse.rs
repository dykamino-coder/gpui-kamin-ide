//! Ряд таблицы как единица повтора (`layout::reuse::GroupCx`).
//!
//! Правка одной ячейки меняет поддерево таблицы, и таблица собирается заново;
//! но ряд, чьё поддерево и входы сборки те же, строит те же элементы ячеек.
//! Входы ряда вне его поддерева названы здесь поимённо: общие для всех рядов
//! (`TableKey`: коробка таблицы, колонки и их группы, геометрия колонок) и
//! свои (`RowKey`: номер ряда, места ячеек в сетке, победившие кромки его
//! ячеек, группа рядов и её край, сдвиг и фон от группы). Чего в ключах нет,
//! то ряд с записью не читает: ряд с объединением по рядам, с полосой фона
//! (ряда, группы, колонки), с transform ряда или группы и ряд вертикальной
//! таблицы собираются как прежде.
//!
//! Что ячейки сообщают таблице в prepaint — кромки сросшихся рамок и фоны
//! ячеек слоя фонов, — запись ряда хранит данными и отдаёт буферам новой
//! сборки (`TableEffects`).

use super::super::anon::TableRow;
use super::super::context::{RowState, RowsOut, TableGeom, TableLayers};
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::reuse::{GroupCx, Shallow};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

mod key;
use key::{GeomKey, Payload, TableEffects, TableKey};
pub(super) use key::RowKey;

/// Row reuse of one table build.
pub(super) struct RowReuse {
    cx: GroupCx,
    /// This build's table key; the key of the records once it equals theirs.
    table: RefCell<Option<Rc<TableKey>>>,
    geom: GeomKey,
    effects: Rc<TableEffects>,
}

impl RowReuse {
    /// Row reuse for the table of `geom`, if its build is one whose rows can
    /// be recorded: a horizontal table without column bands.
    pub(super) fn new(
        geom: &TableGeom<'_>,
        layers: &TableLayers<'_>,
        out: &RowsOut,
        opts: &RenderOpts,
    ) -> Option<Self> {
        let banded = |l: &super::super::context::ColumnLayer<'_>| l.rects.is_some();
        if geom.table_is_vertical
            || geom.e.style.used().wm().vertical()
            || layers.columns.iter().any(banded)
            || layers.groups.iter().any(banded)
        {
            return None;
        }
        let reuse = opts.cx.contracts.reuse.as_ref()?;
        Some(Self {
            cx: reuse.groups(geom.inherited, opts)?,
            table: RefCell::new(None),
            geom: GeomKey::of(geom, layers),
            effects: Rc::new(TableEffects {
                edges: out.table_edges.clone(),
                bgs: out.cell_bgs.clone(),
            }),
        })
    }

    /// The inputs of row `record` at `row_ix`, if the row can be recorded.
    pub(super) fn row_key(
        &self,
        geom: &TableGeom<'_>,
        layers: &TableLayers<'_>,
        row_ix: usize,
        record: &TableRow<'_>,
        state: &RowState<'_>,
    ) -> Option<RowKey> {
        let transformed = |e: &Element| {
            let t = &e.style.used().reset_transforms;
            t.transform.is_some() || t.translate.is_some()
        };
        let grid = &geom.grid.rows[row_ix - 1];
        let carry = &record.carry;
        if state.row_rects.is_some()
            || state.grp_band.is_some()
            || grid.cells.is_empty()
            || grid.cells.iter().any(|slot| slot.rowspan != 1)
            || transformed(record.element)
            || carry.group.is_some_and(transformed)
        {
            return None;
        }
        let edge = layers.group_of.get(&record.element.node_id);
        Some(RowKey {
            row_ix,
            last: row_ix >= geom.row_elements.len(),
            covered: grid.covered,
            slots: grid.cells.iter().map(|s| (s.column, s.colspan)).collect(),
            edges: grid
                .cells
                .iter()
                .map(|s| geom.win_edges.get(&s.element.node_id).copied())
                .collect(),
            group: carry
                .group
                .map(|g| (g.node_id, edge.is_some_and(|e| e.first), edge.is_some_and(|e| e.last))),
            shift: (carry.dx, carry.dy),
            background: carry.background,
        })
    }

    /// The table key of a record is this build's.
    fn same_table(&self, old: &Rc<TableKey>, geom: &TableGeom<'_>, layers: &TableLayers<'_>) -> bool {
        if self.table.borrow().as_ref().is_some_and(|now| Rc::ptr_eq(now, old)) {
            return true;
        }
        let layer = |old: &[Option<Shallow>], now: &[super::super::context::ColumnLayer<'_>]| {
            old.len() == now.len()
                && old.iter().zip(now).all(|(old, now)| match (old, now.element) {
                    (Some(old), Some(now)) => old.same(now),
                    (None, None) => true,
                    _ => false,
                })
        };
        let same = old.geom == self.geom
            && old.table.same(geom.e)
            && layer(&old.columns, &layers.columns)
            && layer(&old.groups, &layers.groups);
        // An equal key is kept: later rows compare by pointer and new records share it.
        if same && self.table.borrow().is_none() {
            self.table.replace(Some(old.clone()));
        }
        same
    }

    /// Push the stand-ins of the cells of `row` if its record fits.
    pub(super) fn replay(
        &self,
        row: &TableRow<'_>,
        key: &RowKey,
        geom: &TableGeom<'_>,
        layers: &TableLayers<'_>,
        opts: &RenderOpts,
        out: &mut RowsOut,
    ) -> bool {
        let same = |old: &dyn Any| {
            old.downcast_ref::<Payload>().is_some_and(|old| {
                old.row == *key
                    && match (&old.group, row.carry.group) {
                        (Some(old), Some(now)) => old.same(now),
                        (None, None) => true,
                        _ => false,
                    }
                    && self.same_table(&old.table, geom, layers)
            })
        };
        self.cx
            .replay(row.element, opts, same, self.effects.clone(), &mut out.cells)
    }

    /// The state before the row's build and the options to build its cells with.
    pub(super) fn mark(&self, opts: &RenderOpts) -> (crate::layout::reuse::RowMark, Box<RenderOpts>) {
        self.cx.mark(opts)
    }

    /// Wrap the cells `out.cells[from..]` just built for `row` for a record.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &self,
        row: &TableRow<'_>,
        key: RowKey,
        mark: crate::layout::reuse::RowMark,
        geom: &TableGeom<'_>,
        layers: &TableLayers<'_>,
        opts: &RenderOpts,
        out: &mut RowsOut,
        from: usize,
    ) {
        let reuse = self.cx.reuse();
        let table = self.table.borrow().clone().unwrap_or_else(|| {
            let layer = |layers: &[super::super::context::ColumnLayer<'_>]| {
                layers
                    .iter()
                    .map(|l| l.element.map(|e| reuse.shallow(e)))
                    .collect()
            };
            Rc::new(TableKey {
                table: reuse.shallow(geom.e),
                columns: layer(&layers.columns),
                groups: layer(&layers.groups),
                geom: GeomKey::of(geom, layers),
            })
        });
        self.table.replace(Some(table.clone()));
        let payload = Rc::new(Payload {
            table,
            row: key,
            group: row.carry.group.map(|g| reuse.shallow(g)),
        });
        let effects = self.effects.clone();
        self.cx
            .record(row.element, mark, opts, payload, effects, &mut out.cells, from);
    }
}