//! Сетка таблицы (ряды, высоты) и рамка таблицы в сросшейся модели.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::table::is_cell;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_strict};
use gpui::{ParentElement, Styled, div, px};
use std::cell::RefCell;
use std::rc::Rc;

pub(super) fn table_grid_box(
    e: &Element,
    inherited: &Computed,
    row_elements: &[&Element],
    tracks: Vec<gpui::GridTrack>,
    table_tall: bool,
    vcx: ValueCx<'_>,
) -> gpui::Div {
    let row_tracks: Option<Vec<gpui::GridTrack>> = match table_tall {
        true if !e.style.wm().vertical() => Some(
            row_elements
                .iter()
                .map(|row| {
                    let cell_h = |c: &Node| match c {
                        Node::Element(cell) if is_cell(cell) => px_strict(cell.style.height),
                        _ => None,
                    };
                    let own = px_strict(row.style.height);
                    match own
                        .into_iter()
                        .chain(row.children.iter().filter_map(cell_h))
                        .fold(None::<f32>, |a, v| Some(a.map_or(v, |x| x.max(v))))
                    {
                        Some(h) => gpui::GridTrack::Pixels(px(h)),
                        None => gpui::GridTrack::Fraction(1.0),
                    }
                })
                .collect(),
        ),
        _ => None,
    };
    // Стол БЕЗ заданной высоты, но с рядами заданной высоты: дорожка такого
    // ряда — `minmax(h, auto)` (CSS 2.1 §17.5.3: высота ряда — большее из
    // заданной и нужной ячейкам), прочие — `auto`. Без дорожек высота ряда
    // не доезжала до сетки вовсе: `tr {height: 50px}` с пустыми ячейками
    // давал ряд в 2 точки паддинга (`table-as-item-cell-percentage-001/003/
    // 004`: стол 100×4 вместо 100×100). Это НЕ вариант «дорожки рядов и без
    // table_tall» (запрет у вызова в `finish/mod.rs`): там авто-ряды
    // становились долями
    // `1fr` с `flex_grow`, и ряды растягивались на высоту растянутого стола;
    // здесь авто-ряд остаётся `auto`, а пол — только у ряда с высотой.
    let row_floors: Option<Vec<gpui::GridTrack>> = (row_tracks.is_none()
        && !e.style.wm().vertical()
        && row_elements
            .iter()
            .any(|r| matches!(r.style.height, Some(Len::Px(h)) if h > 0.0)))
    .then(|| {
        row_elements
            .iter()
            .map(|row| match row.style.height {
                Some(Len::Px(h)) if h > 0.0 => gpui::GridTrack::MinMax(Box::new((
                    gpui::GridTrack::Pixels(px(h)),
                    gpui::GridTrack::Auto,
                ))),
                _ => gpui::GridTrack::Auto,
            })
            .collect()
    });

    let mut grid = if e.style.wm().vertical() {
        // Ряд таблицы — КОЛОНКА сетки: заполнение идёт сверху вниз, ряд за
        // рядом поперёк (css-writing-modes-3 §8, table-progression-*).
        let mut g = div().grid().grid_template_rows(tracks);
        // Table columns consume the assigned physical height (logical inline size).
        if table_tall {
            g = g.flex_grow_1();
        }
        g.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
        g
    } else {
        let mut g = div().grid().grid_template_cols(tracks);
        if let Some(rt) = row_tracks {
            // Сетка обязана занять ВСЮ высоту таблицы: доли рядов считаются
            // от её остатка, а auto-высота ребёнка гибкой колонки — ноль.
            g = g.grid_template_rows(rt).flex_grow_1();
        } else if let Some(rt) = row_floors {
            // Полы рядов (см. `row_floors`); растяжение элемента гибкого
            // контейнера — как в ветке ниже.
            g = g.grid_template_rows(rt);
            if inherited.flex_item {
                g = g.flex_grow_1();
            }
        } else if inherited.flex_item {
            // Стол — элемент гибкого контейнера: высоту, данную ему ростом
            // или растяжением, делят ряды (CSS 2.1 §17.5.3; у сетки
            // `align-content: normal` = stretch тянет auto-ряды), иначе ячейки
            // оставались по содержимому (`table-as-item-stretch-cross-size-2`).
            g = g.flex_grow_1();
        }
        g
    };
    grid.style().table_collapsed_tracks = collapsed_tracks(e, vcx);
    grid
}

pub(super) fn paint_table_border(
    table_edges: Rc<RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    e: &Element,
    bw: [f32; 4],
    outer_win: [f32; 4],
    collapse: bool,
    inherited: &Computed,
    mut outer: gpui::Div,
) -> gpui::Div {
    if collapse && (bw.iter().any(|w| *w > 0.0) || e.style.border_side_styles.contains(&Some(1))) {
        // Рамка самой таблицы — участник разбора конфликтов: её кромки
        // уходят в тот же слой (EdgePainter), линии — внутренние края
        // рамочного места, победившая кромка рисуется наружу.
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour =
            |i: usize| e.style.border_colors[i].resolve(inherited.color.unwrap_or(black));
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
        let side_style =
            |i: usize| e.style.border_side_styles[i].unwrap_or(if bw[i] > 0.0 { 9 } else { 0 });
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        // Линия рамки СТОЛА — та же ЛИНИЯ СЕТКИ, на которой стоят кромки
        // краевых ячеек (§17.6.2: «borders are centered on the grid lines»).
        // Коробка стола вжата внутрь на ПОЛОВИНУ ПОБЕДИВШЕЙ кромки — ровно
        // `outer_win/2` лёг выше в её паддинг, — поэтому и проба вжимается на
        // неё, а не на собственную толщину `bw`. Прежний вжим на `bw` разводил
        // кромку стола и кромки ячеек по РАЗНЫМ группам линий (при
        // `outer_win == bw` — ровно на `bw/2`, то есть на любой рамке от 1.5
        // точек), и разбор конфликта §17.6.2.1 между ними не применялся ни
        // разу: полосы совпадали на экране, а цвет решал порядок рисования —
        // ячейка красилась поверх стола. Снимок `border-conflict-resolution`:
        // нижняя полоса y 191..196 приборных у нас `G67@13 R232@80 G1@312`
        // при `G300@13` у эталона, и 3278 + 1392 = 4670 — смещённых точек нет.
        let half = [
            outer_win[0] / 2.0,
            outer_win[1] / 2.0,
            outer_win[2] / 2.0,
            outer_win[3] / 2.0,
        ];
        outer = outer.child(crate::layout::table::paint::edge_probe(
            table_edges,
            bw,
            colors,
            styles,
            0,
            e.node_id as u32,
            half,
        ));
    }
    outer
}

fn collapsed_tracks(e: &Element, vcx: ValueCx<'_>) -> Option<(bool, Vec<u16>)> {
    // CSS 2.2 section 17.2.1: anonymous rows precede track indexing.
    // Keep the same normalization and header/footer order as table().
    let fixed = crate::layout::table::anon::fixup_table_children(&e.children, vcx);
    let fixed = crate::layout::table::setup::table_rows_fixed(fixed);
    let mut logical_rows = Vec::new();
    crate::layout::table::anon::collect_rows(
        &fixed,
        Some(e),
        crate::layout::table::anon::RowCarry::default(),
        &mut logical_rows,
    );
    let collapsed: Vec<u16> = logical_rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let collapse = crate::layout::table::anon::row_collapsed(row.element, row.carry.group);
            collapse.then_some(if e.style.wm().vertical() && e.style.wm().block_rl() {
                (logical_rows.len() - i - 1) as u16
            } else {
                i as u16
            })
        })
        .collect();
    (!collapsed.is_empty()).then_some((e.style.wm().vertical(), collapsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn collapse_indices_follow_anonymous_rows_and_group_order() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let block = anon_element("div", vec![Node::Element(anon_element("td", vec![]))]);
        let mut group = anon_element("tbody", vec![Node::Element(block)]);
        group.style.collapsed = Some(true);
        group.node_id = 1;
        let mut header = anon_element("thead", vec![Node::Element(anon_element("tr", vec![]))]);
        header.node_id = 2;
        let mut table = anon_element("table", vec![Node::Element(group)]);
        assert_eq!(collapsed_tracks(&table, vcx), Some((false, vec![0])));
        table.children.push(Node::Element(header));
        assert_eq!(collapsed_tracks(&table, vcx), Some((false, vec![1])));
        table.style.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true,
            true,
            table.style.wm().sideways(),
        ));
        assert_eq!(collapsed_tracks(&table, vcx), Some((true, vec![0])));
    }
}
