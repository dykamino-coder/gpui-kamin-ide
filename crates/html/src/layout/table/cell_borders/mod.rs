//! Сросшиеся рамки ячеек (border-collapse): кромки ячейки, ряда, группы рядов, колонки и группы колонок.
// owner: A

use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::ParentElement;
mod columns;
pub(super) use columns::collapsed_col_edges;
pub(super) use columns::collapsed_colgroup_edges;

pub(super) fn collapsed_cell_edge(
    cell: &mut Element,
    cm: &Computed,
    win_edges: &std::collections::HashMap<u64, [f32; 4]>,
    px_of: &impl Fn(Option<Len>) -> f32,
) -> Option<(
    [f32; 4],
    [crate::style::values::value::Color; 4],
    [u8; 4],
    u32,
)> {
    // Толщина в кегельных единицах — из СЛИТОГО стиля, где `em`
    // уже разрешён кеглем ячейки (то же правило, что у `box_style`
    // ниже): сырой `Em` давал нулевую кромку, и ячейка с `border:
    // solid 1em` вовсе не попадала в разбор сросшихся кромок, а
    // рамка рисовалась коробкой — чёрным блоком без разбора
    // конфликтов (`border-conflict-element-001d/001e`).
    let b = {
        let own = cell.style.borders();
        let merged = cm.borders();
        let pick = |o: Option<Len>, m: Option<Len>| match o {
            Some(Len::Px(_)) | None => o,
            _ => m,
        };
        crate::style::computed::Sides {
            top: pick(own.top, merged.top),
            right: pick(own.right, merged.right),
            bottom: pick(own.bottom, merged.bottom),
            left: pick(own.left, merged.left),
        }
    };
    let widths = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
    let black = crate::style::values::value::Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    // Цвет без объявления — `currentColor` (css-backgrounds-3
    // §border-color, initial: currentcolor), а не чёрный: `td.blue
    // {color: blue; border: solid 1em}` красил кромку чёрным.
    let side_colour = |i: usize| {
        cell.style.border_colors[i]
            .or(cell.style.border_color)
            .or(cm.color)
            .unwrap_or(black)
    };
    let colors = [
        side_colour(0),
        side_colour(1),
        side_colour(2),
        side_colour(3),
    ];
    let side_style =
        |i: usize| cell.style.border_side_styles[i].unwrap_or(if widths[i] > 0.0 { 9 } else { 0 });
    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
    // Половина кромки лежит ВНУТРИ ячейки и место занимает
    // (§17.6.2). Кладётся паддингом поверх авторского: проба
    // кромок стоит по паддинг-боксу, и рамкой линия уехала бы
    // внутрь.
    let win = win_edges.get(&cell.node_id).copied().unwrap_or(widths);
    // Авторский отступ в кегельных единицах — из СЛИТОГО стиля
    // (`em` там разрешён кеглем ячейки, как у `box_style`): сырой
    // `Em` падал в ноль, и ячейка сросшейся модели с `padding:
    // 0.5em` теряла отступ целиком — одни полкромки без
    // внутренности (`border-conflict-element-001e`: сетка 100
    // точек вместо 200).
    let merged_pad = cm.padding;
    let half = |i: usize, own: Option<Len>, merged: Option<Len>| {
        let base = match own {
            Some(Len::Px(v)) => v,
            None => 0.0,
            Some(_) => match merged {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            },
        };
        Some(Len::Px(base + win[i] / 2.0))
    };
    cell.style.padding = crate::style::computed::Sides {
        top: half(0, cell.style.padding.top, merged_pad.top),
        right: half(1, cell.style.padding.right, merged_pad.right),
        bottom: half(2, cell.style.padding.bottom, merged_pad.bottom),
        left: half(3, cell.style.padding.left, merged_pad.left),
    };

    cell.style.border_width = Default::default();
    cell.style.border_visible = [None; 4];
    (widths.iter().any(|w| *w > 0.0) || styles.contains(&1)).then_some((
        widths,
        colors,
        styles,
        cell.node_id as u32,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collapsed_row_edges(
    row: &Element,
    col_ix: usize,
    span_cols: u16,
    cols: u16,
    row_style: &Computed,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    px_of: &impl Fn(Option<Len>) -> f32,
    mut d: gpui::Div,
) -> gpui::Div {
    let b = row.style.borders();
    let rw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
    let hidden_row = row.style.border_side_styles.contains(&Some(1));
    if rw.iter().any(|w| *w > 0.0) || hidden_row {
        let start_col = col_ix - span_cols as usize;
        let last_col = col_ix >= cols as usize;
        let widths = [
            rw[0],
            if last_col { rw[1] } else { 0.0 },
            rw[2],
            if start_col == 0 { rw[3] } else { 0.0 },
        ];
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour = |k: usize| {
            row.style.border_colors[k]
                .or(row.style.border_color)
                .or(row_style.color)
                .unwrap_or(black)
        };
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
        let side_style = |k: usize| {
            row.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
        };
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        d = d.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
            widths,
            colors,
            styles,
            4,
            row.node_id as u32,
            [0.0; 4],
        ));
    }
    d
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collapsed_group_edges(
    g: &Element,
    rules_groups: bool,
    col_ix: usize,
    span_cols: u16,
    cols: u16,
    first: bool,
    last: bool,
    inherited: &Computed,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    px_of: &impl Fn(Option<Len>) -> f32,
    mut d: gpui::Div,
) -> gpui::Div {
    let b = g.style.borders();
    let default_w = if rules_groups { 1.0 } else { 0.0 };
    let explicit_top = g.style.border_width.top.is_some() || g.style.border_visible[0].is_some();
    let explicit_bottom =
        g.style.border_width.bottom.is_some() || g.style.border_visible[2].is_some();
    let top_w = if explicit_top {
        px_of(b.top)
    } else {
        default_w
    };
    let bottom_w = if explicit_bottom {
        px_of(b.bottom)
    } else {
        default_w
    };
    // Боковые кромки группы несут крайние ячейки ряда.
    let start_col = col_ix - span_cols as usize;
    let last_col = col_ix >= cols as usize;
    let widths = [
        if first { top_w } else { 0.0 },
        if last_col { px_of(b.right) } else { 0.0 },
        if last { bottom_w } else { 0.0 },
        if start_col == 0 { px_of(b.left) } else { 0.0 },
    ];
    // Нулевая толщина у `hidden` не значит «кромки нет»: скрытая
    // кромка ГАСИТ соседей (§17.6.2.1), поэтому в разбор она
    // обязана попасть наравне с видимыми.
    let hidden_grp = g.style.border_side_styles.contains(&Some(1));
    if widths.iter().any(|w| *w > 0.0) || hidden_grp {
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour = |k: usize| {
            g.style.border_colors[k]
                .or(g.style.border_color)
                .or(g.style.color)
                .or(inherited.color)
                .unwrap_or(black)
        };
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
        let side_style =
            |k: usize| g.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 { 9 } else { 0 });
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        d = d.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
            widths,
            colors,
            styles,
            3,
            g.node_id as u32,
            [0.0; 4],
        ));
    }
    d
}
