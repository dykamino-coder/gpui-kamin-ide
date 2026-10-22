//! Цикл рядов с полосами колонок и групп: подготовка аргументов table_rows.

use crate::dom::Element;
use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;
use crate::layout::table::columns::{col_elements, colgroup_elements, push_col_bands};
use crate::layout::table::rows::*;
use crate::style::values::value::Len;
use gpui::AnyElement;

use super::context::{ColumnLayer, GroupEdge, RowCursor, RowsOut, TableGeom, TableLayers};

pub(super) fn table_rows_with_bands(
    mut geom: TableGeom<'_>,
    rows: Vec<super::anon::TableRow<'_>>,
    out: &mut RowsOut,
    opts: &RenderOpts,
) -> AnyElement {
    let e = geom.e;
    let inherited = geom.inherited;
    let mut layers = column_bands(e, geom.cols, &mut out.under, geom.have_rows, &opts.cx);
    // Ширины рамки самой таблицы: крайние ячейки расползаются фоном на её
    // половину в сросшейся модели.
    // Кегль СВОЙ, а не жёсткие 16 точек: `border: 0.5em` у таблицы с крупным
    // шрифтом давал вчетверо тоньше линию (`border-conflict-element-001d/e`).
    geom.table_em = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };

    let px_of = |l: Option<Len>| geom.px_of(l, opts);
    let table_border = e.style.borders();
    geom.bw = [
        px_of(table_border.top),
        px_of(table_border.right),
        px_of(table_border.bottom),
        px_of(table_border.left),
    ];

    // Легаси-атрибут `rules` (HTML rendering §15.3.10): `groups` даёт
    // группам рядов тонкие кромки по умолчанию.
    let rules_groups = e
        .attr("rules")
        .is_some_and(|v| v.eq_ignore_ascii_case("groups"));
    // Границы ГРУПП РЯДОВ: первый/последний ряд группы несёт её кромку
    // (UA-хинт `rules=groups` — тонкая сплошная, если авторState не задал).
    // Границы снимаются с самих РЯДОВ, а не с детей таблицы: группа может
    // стоять на любом теге через `display: table-row-group`, её ряды — через
    // `display: table-row`, и до фильтра `thead|tbody|tfoot` они не доходили
    // (`border-*-width-applies-to-001/002/003`). Первым и последним рядом
    // группы считаются края её НЕПРЕРЫВНОГО куска в собранном порядке —
    // после перестановки §17.5.3 он уже правильный.
    let mut group_of: std::collections::HashMap<u64, GroupEdge<'_>> =
        std::collections::HashMap::new();
    collect_group_edges(&rows, &mut group_of);
    // Не подавать ряды в обратном порядке для vertical-rl (ряды-колонки от
    // правого края): без обратных охватов rowspan (сетка умеет спан только
    // вперёд) итог хуже. Возвращаться с ЯВНОЙ расстановкой клеток.
    let row_ix = 0i16;
    // Занятость колонок ячейками с rowspan из ПРЕДЫДУЩИХ рядов: без неё
    // номер колонки считался по порядку детей ряда и съезжал — рамки,
    // схлопнутые колонки и пробы фона приписывались не тем колонкам.
    // Алгоритм тот же, что у авторазмещения сетки: занятые клетки
    // пропускаются.
    let occupied: Vec<u16> = vec![0; geom.cols as usize];
    let group_refs: std::collections::HashMap<
        u64,
        crate::paint::effects::transformed_element::RefBox,
    > = std::collections::HashMap::new();
    layers.group_of = group_of;
    layers.rules_groups = rules_groups;
    let mut cursor = RowCursor {
        row_ix,
        col_ix: 0,
        occupied,
        group_refs,
        group_bands: Default::default(),
    };
    table_rows(&rows, &geom, &layers, &mut cursor, out, opts)
}

// Одна запись связывает элемент дорожки и его буфер проб.
pub(super) fn column_bands<'a>(
    e: &'a Element,
    cols: u16,
    under: &mut Vec<AnyElement>,
    have_rows: bool,
    cx: &LayoutCx,
) -> TableLayers<'a> {
    let groups = colgroup_elements(&e.children);
    let columns = col_elements(&e.children);
    let bands = |elements: Vec<Option<&'a Element>>, under: &mut Vec<AnyElement>| {
        let mut layers: Vec<_> = (0..elements.len().max(cols as usize))
            .map(|i| ColumnLayer {
                element: elements.get(i).copied().flatten(),
                rects: None,
            })
            .collect();
        push_col_bands(&mut layers, cols as usize, have_rows, under, cx);
        layers
    };
    TableLayers {
        groups: bands(groups, under),
        columns: bands(columns, under),
        group_of: Default::default(),
        paint_layers: e.style.border_collapse == Some(true),
        rules_groups: false,
    }
}

pub(super) fn collect_group_edges<'a>(
    rows: &[crate::layout::table::anon::TableRow<'a>],
    group_of: &mut std::collections::HashMap<u64, GroupEdge<'a>>,
) {
    let mut i = 0usize;
    while i < rows.len() {
        let Some(g) = rows[i].carry.group else {
            i += 1;
            continue;
        };
        let mut j = i;
        while j < rows.len() && rows[j].carry.group.map(|o| o.node_id) == Some(g.node_id) {
            j += 1;
        }
        for k in i..j {
            group_of.insert(
                rows[k].element.node_id,
                GroupEdge {
                    element: g,
                    first: k == i,
                    last: k + 1 == j,
                },
            );
        }
        i = j;
    }
}

#[cfg(test)]
mod tests;
