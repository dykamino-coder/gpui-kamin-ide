//! Пробы кромок ячейки: полосы фонов рядов, колонок и групп, сросшиеся рамки (CSS 2.1 §17.6.2).

use crate::dom::Element;
use crate::layout::table::{
    collapsed_col_edges, collapsed_colgroup_edges, collapsed_group_edges, collapsed_row_edges,
};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::ParentElement;

#[allow(clippy::too_many_arguments)]
pub(super) fn cell_edge_probes(
    row_ix: i16,
    group_of: &std::collections::HashMap<u64, (&Element, bool, bool)>,
    cols: u16,
    collapse_cells: bool,
    row_elements: &Vec<&Element>,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    col_rects: &Vec<Option<crate::layout::table::paint::RowRects>>,
    col_els: &Vec<Option<&Element>>,
    grp_rects: &Vec<Option<crate::layout::table::paint::RowRects>>,
    grp_els: &Vec<Option<&Element>>,
    rules_groups: bool,
    px_of: &impl Fn(Option<Len>) -> f32,
    row: &Element,
    row_rects: &Option<crate::layout::table::paint::RowRects>,
    grp_band: &Option<crate::layout::table::paint::RowRects>,
    inherited: &Computed,
    row_style: &Computed,
    col_ix: usize,
    span_cols: u16,
    span_rows: u16,
    cell_edge: Option<(
        [f32; 4],
        [crate::style::values::value::Color; 4],
        [u8; 4],
        u32,
    )>,
    cell: &Element,
    d: gpui::Div,
) -> gpui::Div {
    let mut d = d;
    // Полосы фонов рядов и колонок в сросшейся модели начинаются от
    // СЕРЕДИНЫ рамки таблицы (CSS 2.1 §17.6.2): пробы сдвинуты на
    // полкромки — сами ячейки остаются в потоке с полной рамкой.
    // Полкромки таблицы лежит в её паддинге, полкромки ячейки — в
    // паддинге ячейки: полосы фонов встают по месту без поправки.
    let shift = (0.0, 0.0);
    if let Some((widths, colors, styles, doc_ix)) = cell_edge {
        d = d.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
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
        let b = cell.style.borders();
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)]
    };
    if let Some(rects) = row_rects {
        d = d.child(crate::layout::table::paint::cell_rect_probe(
            rects.clone(),
            span_rows == 1,
            shift,
            cell_border,
        ));
    }
    if let Some(rects) = grp_band {
        d = d.child(crate::layout::table::paint::cell_rect_probe(
            rects.clone(),
            span_rows == 1,
            shift,
            cell_border,
        ));
    }
    // Проба и для колонок ячейки: объединённая регистрируется в
    // каждой накрытой колонке — полоса колонки красит её целиком.
    let cell_cols = (col_ix - span_cols as usize)..col_ix;
    let mut probed: Vec<u64> = vec![];
    for i in cell_cols.clone() {
        if let (Some(rects), Some(el)) = (
            col_rects.get(i).and_then(|r| r.clone()),
            col_els.get(i).copied().flatten(),
        ) && !probed.contains(&el.node_id)
        {
            probed.push(el.node_id);
            d = d.child(crate::layout::table::paint::cell_rect_probe(
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
            grp_rects.get(i).and_then(|r| r.clone()),
            grp_els.get(i).copied().flatten(),
        ) && !probed_group.contains(&el.node_id)
        {
            probed_group.push(el.node_id);
            d = d.child(crate::layout::table::paint::cell_rect_probe(
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
    if collapse_cells {
        d = collapsed_row_edges(
            row,
            col_ix,
            span_cols,
            cols,
            row_style,
            table_edges,
            px_of,
            d,
        );
    }
    // Кромки ГРУППЫ РЯДОВ: верх у первого ряда группы, низ у
    // последнего; `rules=groups` даёт тонкую сплошную по умолчанию.
    if collapse_cells && let Some((g, first, last)) = group_of.get(&row.node_id).copied() {
        d = collapsed_group_edges(
            g,
            rules_groups,
            col_ix,
            span_cols,
            cols,
            first,
            last,
            inherited,
            table_edges,
            px_of,
            d,
        );
    }
    // Кромки КОЛОНКИ (рамка <col>/<colgroup>) — участники разбора
    // сросшихся конфликтов (источник между ячейкой и таблицей):
    // ячейка колонки несёт её кромку на совпадающем со спаном
    // колонки краю; верх/низ — только крайние ряды.
    if collapse_cells {
        d = collapsed_col_edges(
            &cell_cols,
            col_els,
            row_ix,
            row_elements,
            inherited,
            table_edges,
            px_of,
            d,
        );
    }
    // Кромки ГРУППЫ КОЛОНОК — свой источник, слабее колонки и сильнее
    // таблицы (§17.6.2.1 п.4). Без него `<colgroup style="border">` с
    // колонками внутри терял рамку целиком: `col_elements` отдаёт
    // внутренние колонки, а сама группа в разбор не попадала.
    if collapse_cells {
        d = collapsed_colgroup_edges(
            cell_cols,
            grp_els,
            row_ix,
            row_elements,
            inherited,
            table_edges,
            px_of,
            d,
        );
    }
    d
}
