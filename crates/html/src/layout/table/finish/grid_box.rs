//! Сетка таблицы (ряды, высоты) и рамка таблицы в сросшейся модели.

use crate::dom::{Element, Node};
use crate::layout::table::is_cell;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{ParentElement, Styled, div, px};

pub(super) fn table_grid_box(
    e: &Element,
    inherited: &Computed,
    row_elements: Vec<&Element>,
    tracks: Vec<gpui::GridTrack>,
    table_tall: bool,
) -> gpui::Div {
    let row_tracks: Option<Vec<gpui::GridTrack>> = match table_tall {
        true if e.style.vertical != Some(true) => Some(
            row_elements
                .iter()
                .map(|row| {
                    let cell_h = |c: &Node| match c {
                        Node::Element(cell) if is_cell(cell) => match cell.style.height {
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        },
                        _ => None,
                    };
                    let own = match row.style.height {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    };
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
    // 004`: стол 100×4 вместо 100×100). Это НЕ откатанный вариант «дорожки
    // рядов и без table_tall» (★ выше): там авто-ряды становились долями
    // `1fr` с `flex_grow`, и ряды растягивались на высоту растянутого стола;
    // здесь авто-ряд остаётся `auto`, а пол — только у ряда с высотой.
    let row_floors: Option<Vec<gpui::GridTrack>> = (row_tracks.is_none()
        && e.style.vertical != Some(true)
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

    if e.style.vertical == Some(true) {
        // Ряд таблицы — КОЛОНКА сетки: заполнение идёт сверху вниз, ряд за
        // рядом поперёк (css-writing-modes-3 §8, table-progression-*).
        let mut g = div().grid().grid_template_rows(tracks);
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
    }
}

pub(super) fn paint_table_border(
    table_edges: std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
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
        let side_colour = |i: usize| {
            e.style.border_colors[i]
                .or(e.style.border_color)
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
            table_edges.clone(),
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
