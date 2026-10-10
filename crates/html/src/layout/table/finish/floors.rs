//! Нижние пределы размеров таблицы (min-*, фиксированная ширина) и ребёнок-сетка.

use crate::dom::Element;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(super) fn table_size_floors(
    e: &Element,
    spacing: (f32, f32),
    cols: u16,
    bw: [f32; 4],
    px_of: impl Fn(Option<Len>) -> f32,
    first_row_widths: Vec<Option<f32>>,
    collapse: bool,
) -> ([f32; 4], Option<Len>, Option<Len>, bool, Option<f32>) {
    let min_fix = |len: Option<Len>, edges: f32| match len {
        // Тегу `<table>` вычитать нельзя: ниже он получает `border_box`
        // (UA-правило), и taffy вычтет рамки с паддингом ВТОРОЙ раз —
        // `min-height-table`: 312 → 300 → содержимое 288 вместо 300. С явным
        // `box-sizing: content-box` порог по спеке и так контентный
        // (`min-max-size-table-content-box`). Зелёная `min-height-table-2`
        // (тот же тег, порог долей) держится ровно на `border_box` без вычета.
        Some(Len::Px(v)) if e.style.border_box != Some(true) && e.tag != "table" => {
            Some(Len::Px((v - edges).max(0.0)))
        }
        other => other,
    };
    let pad = &e.style.padding;
    let pad_px = [
        px_of(pad.top),
        px_of(pad.right),
        px_of(pad.bottom),
        px_of(pad.left),
    ];
    let min_h = min_fix(e.style.min_height, bw[0] + bw[2] + pad_px[0] + pad_px[2]);
    // ПРОБОВАЛИ И ОТКАТИЛИ: поднимать нижнюю грань ширины таблицы до суммы
    // дорожек с зазорами (§17.5.2: «the used width is the greater of the value
    // of width and MIN»). Проба по 19 парам семей `separated-border-model-*` и
    // `fixed-table-layout-02*`: флипов ноль, `separated-border-model-004d`
    // 1.90 -> 1.75. Минимум доезжает, но заданную ширину не перебивает —
    // упирается ниже, в раздачу дорожек внутри сетки.
    let min_w = min_fix(e.style.min_width, bw[1] + bw[3] + pad_px[1] + pad_px[3]);
    // У ТЕГА `<table>` ширина считается по BORDER-BOX (UA-правило
    // css-tables-3: `table { box-sizing: border-box }`), у `display: table` на
    // прочих тегах — контентная. Тесты пишут это прямо: «the width of an
    // HTML/XHTML table is the distance between the left and right table border
    // edges» против «the width of a CSS table … excluding table padding and
    // table borders».
    let table_border_box = e.tag == "table" && e.style.border_box.is_none();
    // Фиксированная раскладка: ширина стола — БОЛЬШЕЕ из `width` и суммы
    // колонок с зазорами (CSS 2.1 §17.5.2.1, «the greater of the value of the
    // 'width' property … and the sum of the column widths (plus cell spacing
    // or borders)»). Коробка брала заявленную ширину всегда, сетка
    // вылезала из неё: `separated-border-model-004d` — фон 9 точек при
    // сетке 42+16+42, `-004c` — 70 при 20·3+20·2. Считается только когда
    // ВСЕ колонки первого ряда известны в точках: иначе сумма не определена.
    // Сросшаяся модель не берётся — её края лежат паддингом `outer_win`.
    let fixed_floor: Option<f32> = match e.style.width {
        Some(Len::Px(w))
            if e.style.table_fixed == Some(true)
                && !collapse
                && e.style.vertical != Some(true)
                && !first_row_widths.is_empty()
                && first_row_widths.iter().all(|c| c.is_some()) =>
        {
            let edges = if table_border_box || e.style.border_box == Some(true) {
                bw[1] + bw[3] + pad_px[1] + pad_px[3]
            } else {
                0.0
            };
            let floor = first_row_widths.iter().flatten().sum::<f32>()
                + spacing.0 * (f32::from(cols) + 1.0)
                + edges;
            (floor > w + 0.5).then_some(floor)
        }
        _ => None,
    };
    (pad_px, min_h, min_w, table_border_box, fixed_floor)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn table_grid_child(
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    e: &Element,
    spacing: (f32, f32),
    from_cols: Vec<Option<f32>>,
    bw: [f32; 4],
    have_rows: bool,
    cells: Vec<AnyElement>,
    grid_box: gpui::Div,
    collapse: bool,
    outer: gpui::Div,
) -> gpui::Div {
    let spacing = (
        if have_rows || !from_cols.is_empty() {
            spacing.0
        } else {
            0.0
        },
        if have_rows { spacing.1 } else { 0.0 },
    );
    let spacing_phys = if e.style.vertical == Some(true) {
        (spacing.1, spacing.0)
    } else {
        spacing
    };
    let mut outer = outer.child(
        grid_box
            // `border-spacing: 2px` — умолчание браузера для таблицы с
            // раздельными рамками. Без него строки идут плотнее, и
            // расхождение копится вниз по таблице.
            .gap_x(px(spacing_phys.0))
            .gap_y(px(spacing_phys.1))
            // Зазор действует и МЕЖДУ краем таблицы и крайними ячейками
            // (CSS 2.1 §17.6.1), не только между ячейками. Эталоны
            // гасят его отрицательным полем на таблице.
            .px(px(spacing_phys.0))
            .py(px(spacing_phys.1))
            .children(cells)
            .into_any_element(),
    );
    if collapse {
        // Граница СЕТКИ выдаётся ВСЕГДА: слой кромок обязан знать, где
        // кончаются дорожки, даже когда своей рамки у таблицы нет. Кромок
        // эта проба не несёт — только координаты.
        //
        // ЗАМЕРЕНО И ОТКАЧЕНО: подменять ею пробу КРОМОК таблицы (нулевые
        // ширины в общем разборе) — CSS2 +21/-25: у таблицы без рамки её
        // коробка совпадает с внешними краями ячеек, и те переставали
        // центрироваться.
        outer = outer.child(crate::layout::table::paint::grid_probe(
            table_edges.clone(),
            bw,
        ));
    }
    outer
}
