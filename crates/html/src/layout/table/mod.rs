//! Таблицы: драйвер раскладки.
// owner: A

use crate::dom::Element;
use crate::layout::table::anon::{RowCarry, collect_rows, fixup_table_children};
use crate::layout::table::columns::col_element_widths;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

pub mod anon;
pub mod columns;
pub mod paint;

mod finish;
mod table_border_widths;
mod table_clipped_content;
mod table_roles;
mod table_spanning_size;
use crate::layout::table::finish::*;
mod cell_borders;
pub(super) mod rows;
use crate::layout::table::cell_borders::*;
mod build_rows;
use build_rows::table_rows_with_bands;
mod zero_cols;
use zero_cols::zero_width_cols;
mod col_measure;
use col_measure::measure_col_widths;
mod setup;
use setup::{
    border_spacing_of, cell_paints_over, row_span_in_group, rows_left_and_cols, table_rows_fixed,
};

/// Таблица.
///
/// Колонки — по содержимому: каждая дорожка это `minmax(min-content, auto)`,
/// последняя забирает остаток (`1fr`). Так ведёт себя и настоящая табличная
/// раскладка: узкие колонки сжимаются до содержимого, широкая тянется.
///
/// Это стало возможно только вместе с патчем произвольных дорожек в GPUI —
/// короткая форма умела ровно «N равных колонок», и таблица из даты и длинного
/// текста разъезжалась пополам.
pub(crate) fn table(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    // Ключевое слово содержимого в `max-height` таблицы горизонтального
    // письма (блочная ось) ведёт себя как начальное `none`: высота таблицы и
    // так не меньше содержимого (css-tables-3 §computing-the-table-height),
    // а общий `apply` превращал пару «`height` + ключевое слово в пределе» в
    // `max_h(height)` с авто-осью, и таблица `height:150px` ужималась до
    // содержимого (`block-size-with-min-or-max-content-table-1a/1b`: эталон
    // держит 150; `support/min-content-max-content.css` — «always treats the
    // 'min-content' and 'max-content' values as the initial value»).
    //
    // Предел в точках таблицу ниже содержимого тоже не ужимает: высота
    // таблицы — большее из заданной (`height`, ограниченной `min/max-height`)
    // и суммы рядов (css-tables-3 §computing-the-table-height; Blink
    // `ComputeTableBlockSize` берёт `max(css_block_size, grid_block_size)`).
    // Предел переходит в саму заданную высоту, а гибкая коробка сетки
    // больше его не видит (`max-height-table`: `max-height: 0` сплющивал
    // ряд 5px в ноль).
    let unlimited;
    let inherited = if inherited.vertical != Some(true)
        && matches!(
            inherited.max_height,
            Some(Len::MinContent)
                | Some(Len::MaxContent)
                | Some(Len::FitContent)
                | Some(Len::Px(_))
        ) {
        let mut c = inherited.clone();
        if let Some(Len::Px(m)) = c.max_height
            && let Some(Len::Px(h)) = c.height
        {
            c.height = Some(Len::Px(h.min(m)));
        }
        c.max_height = None;
        unlimited = c;
        &unlimited
    } else {
        inherited
    };
    // Презентационный `cellspacing="N"`: хинт стоит НИЖЕ авторского
    // `border-spacing`, но выше умолчания браузера. Каскад в вычисленном
    // стиле уже слит, поэтому хинт применяется, только когда значение
    // равно умолчанию тега `<table>` (2px) — авторская двойка при живом
    // атрибуте встречается на порядки реже, чем сами атрибуты.
    let spacing = border_spacing_of(e, opts, inherited);
    // Дети таблицы ЧИНЯТСЯ перед сбором (css-tables-3 §3, fixup):
    // `display: contents` растворяется — его дети идут в таблицу со слитым
    // стилем, — а бесхозные ячейки и текст заворачиваются в анонимный ряд.
    // Без этого содержимое просто пропадало: сборщик рядов видел только
    // настоящие `<tr>` и группы.
    let fixed = fixup_table_children(&e.children);
    // `<thead>` встаёт первым, `<tfoot>` — последним (CSS 2.2 §17.5.3,
    // HTML §14.3.9) СТАБИЛЬНОЙ перестановкой ГРУПП: прочий порядок детей
    // не трогается. Прошлая попытка ломала css-position — она сдвигала
    // ряды и там, где группы уже стояли по порядку.
    let fixed = table_rows_fixed(fixed);
    let mut rows: Vec<(&Element, RowCarry)> = vec![];
    collect_rows(&fixed, Some(e), (0.0, 0.0, None, None), &mut rows);
    // Сколько рядов от i-го до конца ЕГО группы (включая сам ряд): охват
    // ячейки по рядам урезается этим числом, а `rowspan=0` его и берёт (HTML
    // table model: «span all the remaining rows in the row group»). Без
    // урезки сетка заводила НЕЯВНЫЙ ряд вместе с зазором `border-spacing`:
    // у `visibility-collapse-border-spacing-002` второй ряд схлопнут и из
    // сбора выброшен, а `rowspan=2` тянул ячейку на лишние 100 точек.
    // Группа — непрерывный кусок рядов с одним `<tbody>`; ряды прямо в
    // таблице (анонимная группа) идут одним куском.
    let (rows_left, cols) = rows_left_and_cols(&rows);

    // ОДНА сетка на всю таблицу, а не по сетке на строку. Со строками-сетками
    // ширина колонки считалась внутри строки, и соседние строки расходились —
    // заголовок стоял над одним столбцом, значения под другим. Колонки общие
    // только если ячейки живут в общей сетке.
    let cells: Vec<AnyElement> = vec![];
    // Полосы фонов колонок/групп/рядов — ПОД всеми ячейками (§17.5.1 слои
    // 2-5 ниже слоя ячеек; у Blink фоны дорожек и рядов красит сам стол в
    // своей фоновой фазе). Прежде полоса ряда ложилась в сетку перед СВОИМИ
    // ячейками, то есть поверх ячеек предыдущих рядов — для масок разницы
    // нет, а слой кромок (ниже) обязан лечь после ВСЕХ полос.
    let under: Vec<AnyElement> = vec![];
    // Фоны ячеек сросшейся модели — отдельным слоем под кромками
    // (`interact::CellBgs`): так кромки красятся поверх фонов ячеек, но под
    // их содержимым — у Blink сросшиеся кромки идут в фазе
    // `kDescendantBlockBackgroundsOnly` (`box_fragment_painter.cc:952`), а
    // строчное/плавающее/позиционированное содержимое ячеек — позже
    // (`collapsed-border-paint-phase-001`, `collapsed-borders-painting-order-
    // 009/010/012/013`: вложенный стол и инлайн-блок с отрицательным полем
    // накрывались кромками внешнего стола).
    let cell_bgs: crate::layout::table::paint::CellBgs = Default::default();
    // Слои фонов и кромок строятся только при настоящем `border-collapse:
    // collapse` — ровно там, где ниже кладётся `EdgePainter` (у легаси
    // `rules=` без `border-collapse` кромки живут на коробках, и проба фона
    // без своего слоя потеряла бы цвет ячейки).
    let paint_layers = e.style.border_collapse == Some(true);
    // Ячейки, чьё содержимое Blink красит ПОЗЖЕ сросшихся кромок (строчный
    // уровень, флоаты, позиционированные, контексты наложения — фазы после
    // `kDescendantBlockBackgroundsOnly`), идут в сетку ПОСЛЕ слоя кромок;
    // ячейки с одним блочным содержимым — до него, и их блочные потомки
    // (в том числе вложенный блочный стол с его кромками) остаются под
    // кромками внешнего (`collapsed-borders-painting-order-007/008/011`,
    // `collapsed-border-paint-phase-002`). Разделение по ЯЧЕЙКЕ, а не по
    // потомку: слоёв фаз у нас нет, а порядок детей сетки при явной
    // расстановке на раскладку не влияет.
    let cells_over: Vec<AnyElement> = vec![];
    // Наследуемые свойства САМОЙ таблицы обязаны дойти до ячеек: `inherited`
    // — это стиль её РОДИТЕЛЯ, и всё объявленное на теге таблицы
    // (`white-space`, шрифт, цвет) шло мимо. Видно было по сохранённым
    // пробелам: в ячейке они схлопывались, хотя на таблице стоял
    // `white-space: break-spaces`.
    //
    // С первого раза правка была в минус (ломалась арабская вязь) — но ломал
    // её свой замер ширин, который мерил текст ячейки отдельно от раскладки.
    // Со снятым замером она проходит чисто.
    let row_elements: Vec<&Element> = rows.iter().map(|(r, _)| *r).collect();
    // Заявленные ширины ячеек по КОЛОНКАМ: ширина ячейки в таблице задаёт
    // колонку, а не свою коробку (CSS 2.1 §17.5.2.2) — колонка не уже
    // содержимого (пол min-content), процентная забирает долю остатка.
    let mut col_widths: Vec<(Option<f32>, Option<f32>)> = vec![(None, None); cols as usize];
    let table_font = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let table_family = inherited.font_family.clone().unwrap_or_default();
    let (from_cols, cols_collapsed, cols_pct) =
        col_element_widths(&e.children, table_font, &table_family);
    // §17.5.2.1: при фиксированной раскладке и ЗАДАННОЙ ширине стола
    // колонка, которой места уже не осталось, получает НОЛЬ — и ячейка в ней
    // не вправе распирать дорожку своим отступом, иначе она вылезает за край
    // стола (`fixed-table-layout-025/028..031`). Ширина стола АВТО в этот
    // гейт не попадает: на ней держится семья `margin-*-applies-to-*`, на
    // которой умерли три прошлых захода (см. записи ниже по ячейке).
    let zero_cols = zero_width_cols(e, cols, &row_elements, &from_cols);
    let busy: Vec<u16> = vec![0; cols as usize];
    let (win_edges, outer_win) = table_border_widths::resolve(
        e,
        &row_elements,
        &rows.iter().map(|(_, carry)| carry.3).collect::<Vec<_>>(),
        &rows_left,
        cols,
        table_font,
        &table_family,
    );
    // Вертикальность САМОЙ таблицы: `inherited` внутри цикла рядов
    // перекрыт слоем группы строк (`<tbody>` с письмом травил гейты,
    // table-progression-htb-001 — письмо к рядам и группам НЕ применяется).
    let table_is_vertical = e.style.vertical == Some(true) || inherited.vertical == Some(true);
    measure_col_widths(
        e,
        inherited,
        &rows_left,
        &row_elements,
        &mut col_widths,
        table_font,
        table_family,
        busy,
        &win_edges,
    );
    // Слой ГРУПП КОЛОНОК и слой КОЛОНОК — две полосы, снизу вверх (§17.5.1:
    // «the next layer contains the column groups… on top of the column groups
    // are the areas representing the column boxes»). У каждой свой буфер
    // проб: площадь группы шире колоночной, и `background-position` у них
    // разный. Обе идут ПЕРЕД рядами: колонка рисуется ниже ряда
    // (css-tables-3 §layers).
    table_rows_with_bands(
        e,
        opts,
        inherited,
        spacing,
        rows,
        rows_left,
        cols,
        cells,
        under,
        cell_bgs,
        paint_layers,
        cells_over,
        row_elements,
        col_widths,
        table_font,
        from_cols,
        cols_collapsed,
        cols_pct,
        zero_cols,
        win_edges,
        outer_win,
        table_is_vertical,
    )
}

pub(super) fn is_cell(e: &Element) -> bool {
    table_roles::is_cell(e)
}
