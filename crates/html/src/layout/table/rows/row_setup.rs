//! Подготовка ряда и ячейки в цикле table_rows: пробы полос ряда/группы, дорожка пустого ряда,
//! слитый стиль ячейки и её охваты (colspan/rowspan, схлопнутые колонки).

use crate::document::state::ValueCx;
use crate::dom::pseudo_line_layers;
use crate::dom::{Element, Node};
use crate::layout::build_contracts::Layer;
use crate::layout::cx::LayoutCx;
use crate::layout::table::anon::html_cell;
use crate::layout::table::rows::gather_text;
use crate::layout::table::{is_cell, row_span_in_group};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Overflow};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, Styled, div, px};

pub(super) struct CellSpans {
    pub span_cols: u16,
    pub span_rows: u16,
    pub spans_collapsed: bool,
    pub clipped: bool,
}

pub(super) fn cell_spans(
    row_ix: i16,
    rows_left: &[usize],
    cols_collapsed: &[bool],
    col_ix: usize,
    cell: &Element,
) -> CellSpans {
    let st = cell.style.computed();
    // Объединение ячеек: без него ячейка занимала одну дорожку, и всё
    // правее неё съезжало на колонку влево.
    let span_cols: u16 = cell
        .attr("colspan")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    // Охват по рядам урезан до конца группы, `rowspan=0` — до конца
    // группы (см. `rows_left`): лишний охват создавал неявный ряд
    // сетки с зазором (`visibility-collapse-border-spacing-002`).
    let span_rows: u16 = row_span_in_group(
        cell,
        rows_left
            .get(usize::try_from(row_ix - 1).unwrap_or(0))
            .copied()
            .unwrap_or(1),
    )
    .min(usize::from(u16::MAX)) as u16;
    // Фон и рамка строки переносятся на её ячейки: своей строки как
    // элемента больше нет, а зебра и разделители нужны.
    // Обрезка снимается С САМОЙ ячейки и вешается на её содержимое.
    // Причина: раскладка под нами, увидев `overflow: hidden`, снимает
    // с элемента автоминимум — по CSS так и надо, — а размера от
    // таблицы ячейка не получает, и вся она схлопывается в ноль
    // (`flexbox_rowspan-overflow` рисовал пустую страницу).
    // Коробка ячейки при этом обрезать содержимое не перестаёт.
    // Объединённая ячейка ЧЕРЕЗ схлопнутую колонку обрезается по
    // урезанной ширине (css-tables-3 §visibility-collapse-cell-
    // rendering): содержимое не расталкивает оставшиеся колонки.
    let spans_collapsed = span_cols > 1
        && (col_ix..col_ix + span_cols as usize)
            .any(|i| cols_collapsed.get(i).copied().unwrap_or(false));
    let clipped =
        [st.overflow_x, st.overflow_y].contains(&Some(Overflow::Hidden)) || spans_collapsed;
    CellSpans {
        span_cols,
        span_rows,
        spans_collapsed,
        clipped,
    }
}

pub(super) struct RowBands {
    pub row_rects: Option<crate::layout::table::probes::RowRects>,
    pub grp_band: Option<crate::layout::table::probes::RowRects>,
}

pub(super) fn row_band_probes(
    group_bands: &mut std::collections::HashMap<u64, crate::layout::table::probes::RowRects>,
    under: &mut Vec<AnyElement>,
    group_of: &std::collections::HashMap<u64, crate::layout::table::context::GroupEdge<'_>>,
    row: &Element,
    carry: crate::layout::table::anon::RowCarry<'_>,
    cx: &LayoutCx,
) -> RowBands {
    let rs = row.style.computed();
    // Фон ряда КАРТИНКОЙ (css-tables-3 §drawing-backgrounds): рисуется в
    // ЯЧЕЙКАХ, непрерывно от начала ряда, зазоры остаются чистыми.
    // Полоса на весь ряд несёт слой фона, но обрезает его прямоугольниками
    // ячеек, снятыми пробами этого кадра (`RowRects`).
    let row_rects: Option<crate::layout::table::probes::RowRects> =
        (!crate::layout::table::anon::row_collapsed(row, carry.group)
            && (row.style.computed().bg_image.is_some()
                || row.style.computed().gradient_raw.is_some()
                || (row.style.computed().background.is_some() && row.style.computed().bg_clip.is_some())
                || !row.style.computed().shadows.is_empty()))
        .then(crate::layout::table::probes::RowRects::default);
    // Фон ГРУППЫ рядов красится так же, как фон ряда (§17.5.1, слой 3):
    // полоса идёт от левого края крайней левой колонки до правого края
    // крайней правой и обрезается прямоугольниками ячеек. Своей коробки у
    // группы в сетке нет, поэтому картинка и градиент пропадали вовсе —
    // рисовался только сплошной цвет, который течёт вниз наследованием.
    // Обводка группы (`outline` — «Applies to: all elements», css-ui-4)
    // тоже идёт полосой: своей коробки у группы нет, а охват её ячеек
    // полоса уже считает (`visibility-collapse-border-spacing-001`:
    // `tbody { outline: 10px }` не рисовался вовсе).
    let grp_band: Option<crate::layout::table::probes::RowRects> = carry.group.and_then(|g| {
        (g.style.computed().collapsed != Some(true)
            && (g.style.computed().bg_image.is_some()
                || g.style.computed().gradient_raw.is_some()
                || (g.style.computed().background.is_some() && g.style.computed().bg_clip.is_some())
                || !g.style.computed().shadows.is_empty()
                || g.style.computed().outline.is_some()))
        .then(|| group_bands.entry(g.node_id).or_default().clone())
    });
    if let (Some(rects), Some(g)) = (&grp_band, carry.group)
        && group_of.get(&row.node_id).is_some_and(|edge| edge.first)
    {
        let mut band_style = g.style.cascaded().clone();
        if band_style.bg_image.is_none() {
            band_style.bg_image = band_style.gradient_raw.clone();
        }
        // Полоса только с обводкой не дублирует цвет, который несут ячейки.
        if band_style.bg_image.is_none() && g.style.computed().shadows.is_empty() && g.style.computed().bg_clip.is_none()
        {
            band_style.background = None;
        }
        let band = Layer::cell_band(rects.clone(), band_style, cx);
        under.push(cx.contracts.build.layer(band));
    }
    if let Some(rects) = &row_rects {
        // Градиент ряда идёт слоем-картинкой: источник понимает записи
        // `linear-gradient(...)` и растрирует их сам.
        let mut band_style = row.style.computed().as_ref().clone();
        if band_style.bg_image.is_none() {
            band_style.bg_image = band_style.gradient_raw.clone();
        }
        let band = Layer::cell_band(rects.clone(), band_style, cx);
        under.push(cx.contracts.build.layer(band));
    }
    RowBands {
        row_rects,
        grp_band,
    }
}

pub(super) fn push_empty_row_track(
    row_ix: i16,
    occupied: &[u16],
    e: &Element,
    cols: u16,
    cells: &mut Vec<AnyElement>,
    row: &Element,
) {
    // Ряд БЕЗ ячеек с заданной высотой держит свою дорожку (CSS 2.1
    // §17.5.3: высота ряда — не меньше его `height`). Элементов сетки у
    // него нет, и дорожка пропадала: `border-collapse-empty-row` терял
    // 2/5/10 точек пустых рядов, и шаг рядов расходился с эталоном
    // (20+2 против 10+12). Заглушка на всю ширину ставится только когда
    // ряд не накрыт охватом сверху — иначе авторазмещение унесло бы её
    // в следующий ряд.
    if !e.style.cascaded().wm().vertical()
        && !row
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(c) if is_cell(c)))
        && occupied.iter().all(|o| *o == 0)
        && let Some(Len::Px(h)) = row.style.computed().height
        && h > 0.0
    {
        let mut ph = div().h(px(h)).col_span(cols);
        if !e.style.cascaded().wm().vertical() {
            ph = ph.col_start(1).row_start(row_ix);
        }
        cells.push(ph.into_any_element());
    }
}

pub(super) fn cell_cascaded_style(
    e: &Element,
    row_style: &BoxStyle,
    cell: &Element,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    let st = cell.style.computed();
    let mut cm = box_style(row_style, &cell.style, vcx);
    pseudo_line_layers::install(cell, &mut cm, vcx);
    // `vertical-align` is not inherited (CSS 2.1 §10.8.1): only `td`/
    // `th` take their row's value, through the UA rule
    // `vertical-align: inherit` (HTML §15.3.9). A generic
    // `display: table-cell` box keeps its own value or the initial
    // `baseline` (`vertical-align-applies-to-*`: a row group's
    // `bottom` must not move the cell's content).
    if cell.style.own_vertical_align().is_none() && !html_cell(cell) {
        cm.vertical_align = None;
    }
    // Потолок вертикальной ячейки режет доступное место её
    // ортогонального потока — как у блока (см. ortho_limit в
    // element): стопка глифов переносится на следующую колонку по
    // нему (table-cell-002: td vertical-rl с max-height 100 —
    // зелёный квадрат из двух колонок).
    // Высота в `ch` разрешается позже, с письмом ряда (`cell_heights`).
    if cell.style.cascaded().wm().vertical()
        && cm.ortho_limit.is_none()
        && !matches!(cell.style.cascaded().height, Some(Len::Ch(_)))
        && let Some(Len::Px(h)) = st.height.or(st.max_height)
    {
        cm.ortho_limit = Some(h);
    }
    // Инлайн-мера ячейки ПАРАЛЛЕЛЬНОЙ таблицы — дорожка КОЛОНКИ, не стол. Предел, приехавший сверху
    // наследованием, тут запасной по §7.3, а запас ставится ТОЛЬКО на
    // место неопределённого инлайн-места — у ячейки оно определённое
    // (css-tables-3 §computing-column-measures). Blink тем же
    // условием: `space_utils.h:36` выходит при
    // `IsParallelWritingMode(таблица, ячейка)`, а
    // `table_layout_utils.cc:1363` даёт ячейке место из
    // `column_locations`. Пометка не гасит предел (он ещё нужен
    // потолком: колонка не шире стола), а меняет способ замера —
    // см. `col_min` в `paragraph()`.
    //
    // Гейт узкий намеренно: письмо должно стоять на САМОМ столе
    // (`e.style.vertical`, тот же гейт, что у транспонирования
    // решётки и `spacing_phys`), и предел должен уже быть — иначе
    // ничего не меняется. У анонимной обёртки `display: table-cell`
    // (`anon_element("table", …)`, стиль `Computed::default()`)
    // `e.style.vertical` пуст, поэтому семья `line-box-direction-*`
    // гейтом не задевается.
    if e.style.cascaded().wm().vertical() && cm.ortho_limit.is_some() {
        cm.ortho_col = true;
    }
    cm
}

pub(super) fn hides_empty_cell(
    e: &Element,
    row_style: &BoxStyle,
    cell: &Element,
    vcx: ValueCx<'_>,
) -> bool {
    box_style(row_style, &cell.style, vcx).empty_cells_hide == Some(true)
        && e.style.cascaded().border_collapse != Some(true)
        && {
            let mut текст = String::new();
            gather_text(&cell.children, &mut текст);
            текст.trim().is_empty() && !cell.children.iter().any(|n| matches!(n, Node::Element(_)))
        }
}

#[cfg(test)]
mod tests;
