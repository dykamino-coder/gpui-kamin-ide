//! Пробы кромок ячейки: полосы фонов рядов, колонок и групп, сросшиеся рамки (CSS 2.1 §17.6.2).

use crate::dom::Element;
use crate::layout::table::{
    collapsed_col_edges, collapsed_colgroup_edges, collapsed_group_edges, collapsed_row_edges,
};
use crate::style::values::value::px_lossy_zero;
use gpui::ParentElement;

use super::super::context::{CellEdge, RowCursor, RowState, RowsOut, TableGeom, TableLayers};
use crate::layout::options::RenderOpts;

pub(super) struct CellProbe<'a> {
    pub span_cols: u16,
    pub span_rows: u16,
    pub cell_edge: Option<CellEdge>,
    pub cell: &'a Element,
    pub d: gpui::Div,
}

pub(super) fn cell_edge_probes(
    geom: &TableGeom<'_>,
    layers: &TableLayers<'_>,
    cursor: &RowCursor,
    out: &mut RowsOut,
    row_state: &RowState<'_>,
    probe: CellProbe<'_>,
    opts: &RenderOpts,
) -> gpui::Div {
    let CellProbe {
        span_cols,
        span_rows,
        cell_edge,
        cell,
        d,
    } = probe;
    let vcx = opts.cx.vcx();
    let px_of = |l| geom.px_of(l, opts);
    let mut d = d;
    // Полосы фонов рядов и колонок в сросшейся модели начинаются от
    // СЕРЕДИНЫ рамки таблицы (CSS 2.1 §17.6.2): пробы сдвинуты на
    // полкромки — сами ячейки остаются в потоке с полной рамкой.
    // Полкромки таблицы лежит в её паддинге, полкромки ячейки — в
    // паддинге ячейки: полосы фонов встают по месту без поправки.
    let shift = (0.0, 0.0);
    if let Some(CellEdge {
        widths,
        colors,
        styles,
        doc_ix,
    }) = cell_edge
    {
        d = d.child(crate::layout::table::probes::edge_probe(
            out.table_edges.clone(),
            widths,
            colors,
            styles,
            5,
            doc_ix,
            [0.0; 4],
        ));
    }
    // Рамка ячейки — обратно в границы: канвас пробы лежит внутри
    // неё, а фон полосы идёт по внешним краям (§17.5.1).
    let cell_border = {
        let b = cell.style.cascaded().borders();
        let px_of = px_lossy_zero;
        [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)]
    };
    if let Some(rects) = &row_state.row_rects {
        d = d.child(crate::layout::table::probes::cell_rect_probe(
            rects.clone(),
            span_rows == 1,
            shift,
            cell_border,
        ));
    }
    if let Some(rects) = &row_state.grp_band {
        d = d.child(crate::layout::table::probes::cell_rect_probe(
            rects.clone(),
            span_rows == 1,
            shift,
            cell_border,
        ));
    }
    // Проба и для колонок ячейки: объединённая регистрируется в
    // каждой накрытой колонке — полоса колонки красит её целиком.
    let cell_cols = (cursor.col_ix - span_cols as usize)..cursor.col_ix;
    let mut probed: Vec<u64> = vec![];
    for i in cell_cols.clone() {
        if let (Some(rects), Some(el)) = (
            layers.columns.get(i).and_then(|r| r.rects.clone()),
            layers.columns.get(i).and_then(|r| r.element),
        ) && !probed.contains(&el.node_id)
        {
            probed.push(el.node_id);
            d = d.child(crate::layout::table::probes::cell_rect_probe(
                rects,
                span_cols == 1,
                shift,
                cell_border,
            ));
        }
    }
    // Та же проба для слоя ГРУППЫ: её коробка идёт «from the left
    // edge of its leftmost column to the right edge of its rightmost
    // column» (§17.5.1) — площадь шире колоночной, поэтому буфер
    // свой.
    let mut probed_group: Vec<u64> = vec![];
    for i in cell_cols.clone() {
        if let (Some(rects), Some(el)) = (
            layers.groups.get(i).and_then(|r| r.rects.clone()),
            layers.groups.get(i).and_then(|r| r.element),
        ) && !probed_group.contains(&el.node_id)
        {
            probed_group.push(el.node_id);
            d = d.child(crate::layout::table::probes::cell_rect_probe(
                rects,
                span_cols == 1,
                shift,
                cell_border,
            ));
        }
    }
    // Кромки РЯДА (border на <tr>) — участник разбора сросшихся
    // конфликтов (CSS 2.1 §17.6.2.1: ячейка > ряд > группа >
    // колонка > таблица); в раздельной модели рамки ряда не
    // действуют вовсе (§17.6.1) — сюда попадает только collapse.
    if geom.collapse_cells {
        // Rowspan is already clipped to its row group by cell_spans.
        let last_row = (span_rows > 1).then(|| {
            geom.row_elements
                .get(cursor.row_ix as usize + span_rows as usize - 2)
                .copied()
                .unwrap_or(row_state.row)
        });
        d = collapsed_row_edges(
            row_state.row,
            cursor.col_ix,
            span_cols,
            geom.cols,
            &row_state.row_style,
            row_state.inherited,
            last_row,
            &out.table_edges,
            &px_of,
            d,
            vcx,
        );
    }
    // Кромки ГРУППЫ РЯДОВ: верх у первого ряда группы, низ у
    // последнего; `rules=groups` даёт тонкую сплошную по умолчанию.
    if geom.collapse_cells
        && let Some(super::super::context::GroupEdge {
            element: g,
            first,
            last,
        }) = layers.group_of.get(&row_state.row.node_id).copied()
    {
        d = collapsed_group_edges(
            g,
            layers.rules_groups,
            cursor.col_ix,
            span_cols,
            geom.cols,
            first,
            last,
            row_state.inherited,
            &out.table_edges,
            &px_of,
            d,
        );
    }
    // Кромки КОЛОНКИ (рамка <col>/<colgroup>) — участники разбора
    // сросшихся конфликтов (источник между ячейкой и таблицей):
    // ячейка колонки несёт её кромку на совпадающем со спаном
    // колонки краю; верх/низ — только крайние ряды.
    if geom.collapse_cells {
        d = collapsed_col_edges(
            &cell_cols,
            &layers.columns,
            cursor.row_ix,
            &geom.row_elements,
            row_state.inherited,
            &out.table_edges,
            &px_of,
            d,
        );
    }
    // Кромки ГРУППЫ КОЛОНОК — свой источник, слабее колонки и сильнее
    // таблицы (§17.6.2.1 п.4). Без него `<colgroup style="border">` с
    // колонками внутри терял рамку целиком: `col_elements` отдаёт
    // внутренние колонки, а сама группа в разбор не попадала.
    if geom.collapse_cells {
        d = collapsed_colgroup_edges(
            cell_cols,
            &layers.groups,
            cursor.row_ix,
            &geom.row_elements,
            row_state.inherited,
            &out.table_edges,
            &px_of,
            d,
        );
    }
    d
}
