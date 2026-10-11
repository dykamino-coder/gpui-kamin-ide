//! Цикл рядов с полосами колонок и групп: подготовка аргументов table_rows.

use crate::dom::Element;
use crate::layout::table::columns::{col_elements, colgroup_elements, push_col_bands};
use crate::layout::table::rows::*;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(super) fn table_rows_with_bands(
    e: &Element,
    opts: &RenderOpts,
    inherited: &Computed,
    spacing: (f32, f32),
    rows: Vec<(
        &Element,
        (
            f32,
            f32,
            Option<crate::style::values::value::Color>,
            Option<&Element>,
        ),
    )>,
    rows_left: Vec<usize>,
    cols: u16,
    cells: Vec<AnyElement>,
    mut under: Vec<AnyElement>,
    cell_bgs: std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    paint_layers: bool,
    cells_over: Vec<AnyElement>,
    row_elements: Vec<&Element>,
    col_widths: Vec<(Option<f32>, Option<f32>)>,
    table_font: f32,
    from_cols: Vec<Option<f32>>,
    cols_collapsed: Vec<bool>,
    cols_pct: Vec<Option<f32>>,
    zero_cols: Vec<bool>,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    outer_win: [f32; 4],
    table_is_vertical: bool,
) -> AnyElement {
    let (grp_els, col_els, grp_rects, col_rects, have_rows) =
        column_bands(e, opts, cols, &mut under, &row_elements);
    // Ширины рамки самой таблицы: крайние ячейки расползаются фоном на её
    // половину в сросшейся модели.
    // Кегль СВОЙ, а не жёсткие 16 точек: `border: 0.5em` у таблицы с крупным
    // шрифтом давал вчетверо тоньше линию (`border-conflict-element-001d/e`).
    let table_em = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let table_family = inherited.font_family.clone().unwrap_or_default();
    let px_of = |l: Option<Len>| crate::text::metrics::spacing_px(l, &table_family, table_em);
    let table_border = e.style.borders();
    let bw = [
        px_of(table_border.top),
        px_of(table_border.right),
        px_of(table_border.bottom),
        px_of(table_border.left),
    ];
    let table_edges = crate::layout::table::paint::cell_edges_for(e.node_id ^ opts.doc_salt);
    let collapse_cells = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
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
    let mut group_of: std::collections::HashMap<u64, (&Element, bool, bool)> =
        std::collections::HashMap::new();
    collect_group_edges(&rows, &mut group_of);
    // ПРОБОВАЛИ И ОТКАТИЛИ: подавать ряды в обратном порядке для vertical-rl
    // (ряды-колонки от правого края). Без обратных охватов rowspan (сетка
    // умеет спан только вперёд) -001 пары ушли 1.02 → 1.31; -003 выиграла
    // 1.18 → 0.82 — нетто минус. Возвращаться с ЯВНОЙ расстановкой клеток.
    let row_ix = 0i16;
    // Занятость колонок ячейками с rowspan из ПРЕДЫДУЩИХ рядов: без неё
    // номер колонки считался по порядку детей ряда и съезжал — рамки,
    // схлопнутые колонки и пробы фона приписывались не тем колонкам.
    // Алгоритм тот же, что у авторазмещения сетки: занятые клетки
    // пропускаются.
    let occupied: Vec<u16> = vec![0; cols as usize];
    let group_refs: std::collections::HashMap<
        u64,
        crate::paint::effects::transformed_element::RefBox,
    > = std::collections::HashMap::new();
    let tbl_style: &Computed = inherited;
    table_rows(
        rows,
        row_ix,
        occupied,
        opts,
        under,
        group_of,
        inherited,
        e,
        cols,
        cells,
        zero_cols,
        rows_left,
        cols_collapsed,
        collapse_cells,
        win_edges,
        table_is_vertical,
        table_font,
        &table_family,
        paint_layers,
        cell_bgs,
        row_elements,
        table_edges,
        col_rects,
        col_els,
        grp_rects,
        grp_els,
        rules_groups,
        group_refs,
        tbl_style,
        cells_over,
        spacing,
        from_cols,
        col_widths,
        cols_pct,
        bw,
        outer_win,
        have_rows,
        px_of,
    )
}

pub(super) fn column_bands<'a>(
    e: &'a Element,
    opts: &RenderOpts,
    cols: u16,
    under: &mut Vec<AnyElement>,
    row_elements: &[&Element],
) -> (
    Vec<Option<&'a Element>>,
    Vec<Option<&'a Element>>,
    Vec<Option<crate::layout::table::paint::RowRects>>,
    Vec<Option<crate::layout::table::paint::RowRects>>,
    bool,
) {
    let grp_els = colgroup_elements(&e.children);
    let col_els = col_elements(&e.children);
    let mut grp_rects: Vec<Option<crate::layout::table::paint::RowRects>> =
        vec![None; cols as usize];
    let mut col_rects: Vec<Option<crate::layout::table::paint::RowRects>> =
        vec![None; cols as usize];
    let have_rows = !row_elements.is_empty();
    push_col_bands(&grp_els, opts.doc_salt, have_rows, &mut grp_rects, under);
    push_col_bands(&col_els, opts.doc_salt, have_rows, &mut col_rects, under);
    (grp_els, col_els, grp_rects, col_rects, have_rows)
}

pub(super) fn collect_group_edges<'a>(
    rows: &[(
        &'a Element,
        (
            f32,
            f32,
            Option<crate::style::values::value::Color>,
            Option<&'a Element>,
        ),
    )],
    group_of: &mut std::collections::HashMap<u64, (&'a Element, bool, bool)>,
) {
    let mut i = 0usize;
    while i < rows.len() {
        let Some(g) = rows[i].1.3 else {
            i += 1;
            continue;
        };
        let mut j = i;
        while j < rows.len() && rows[j].1.3.map(|o| o.node_id) == Some(g.node_id) {
            j += 1;
        }
        for k in i..j {
            group_of.insert(rows[k].0.node_id, (g, k == i, k + 1 == j));
        }
        i = j;
    }
}
