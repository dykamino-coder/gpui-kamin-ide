//! Таблицы: драйвер раскладки.
// owner: A

use crate::render::*;

pub mod anon;
pub mod columns;
pub mod paint;

/// Таблица.
///
/// Колонки — по содержимому: каждая дорожка это `minmax(min-content, auto)`,
/// последняя забирает остаток (`1fr`). Так ведёт себя и настоящая табличная
/// раскладка: узкие колонки сжимаются до содержимого, широкая тянется.
///
/// Это стало возможно только вместе с патчем произвольных дорожек в GPUI —
/// короткая форма умела ровно «N равных колонок», и таблица из даты и длинного
/// текста разъезжалась пополам.
pub(crate) mod table_roles;
pub(crate) mod table_border_widths;
pub(crate) mod table_spanning_size;
pub(crate) mod table_clipped_content;
pub(crate) mod finish;
pub(crate) use crate::layout::table::finish::*;
pub(crate) mod rows;
pub(crate) use crate::layout::table::rows::*;
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
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent) | Some(Len::Px(_))
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
    let cell_spacing_attr = e
        .attr("cellspacing")
        .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok());
    let ua_default = matches!(
        e.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let border_spacing = match (cell_spacing_attr, ua_default, e.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => Some((Some(Len::Px(v)), Some(Len::Px(v)))),
        _ => e.style.border_spacing,
    };
    // Раздельные рамки — умолчание; при `collapse` зазора между ячейками нет.
    let spacing = match (e.style.border_collapse, border_spacing) {
        (Some(true), _) => (0.0, 0.0),
        (None, _) if e.attr("rules").is_some() => (0.0, 0.0),
        // Заданный `border-spacing` перекрывает умолчание браузера в 2px.
        // Шрифтовые единицы разрешаются по кеглю САМОЙ таблицы: `1em` роняло
        // зазор в ноль, и вся подсемья Хикси с `border-spacing: 1em`
        // расходилась с эталоном ровно на зазор.
        (_, Some((x, y))) => {
            let em = atom_base_font(inherited, opts);
            let px_of = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_) | Len::Ic(_))) => {
                    crate::metrics::fallback_len_px(l, "", em).unwrap_or(0.0)
                }
                _ => 0.0,
            };
            (px_of(x), px_of(y))
        }
        // Начальное значение `border-spacing` — НОЛЬ: два пикселя — это
        // умолчание браузера для ТЕГА `<table>`, и оно приходит сюда
        // каскадом из своего стилевого листа. `div` с `display: table`
        // зазора не имеет.
        _ => (0.0, 0.0),
    };
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
    let fixed = {
        // Роль группы задаётся ТЕГОМ ИЛИ `display` (§17.5.3): `div` с
        // `table-header-group` встаёт первым так же, как `<thead>`.
        let kind_of = |g: &Element| -> Option<u8> {
            match g.tag.as_str() {
                "thead" => Some(0),
                "tbody" => Some(1),
                "tfoot" => Some(2),
                _ => g.style.row_group_kind,
            }
        };
        let first_with = |k: u8| -> Option<u64> {
            fixed.iter().find_map(|n| match n {
                Node::Element(g) if kind_of(g) == Some(k) => Some(g.node_id),
                _ => None,
            })
        };
        // Заголовочной и подвальной становится только ПЕРВАЯ группа
        // своего рода; последующие — обычные группы рядов.
        let head = first_with(0);
        let foot = first_with(2);
        let key = |n: &Node| match n {
            Node::Element(g) if Some(g.node_id) == head => 0u8,
            Node::Element(g) if Some(g.node_id) == foot => 2,
            _ => 1,
        };
        let ordered = fixed.windows(2).all(|w| key(&w[0]) <= key(&w[1]));
        if ordered {
            fixed
        } else {
            let mut sorted = fixed;
            sorted.sort_by_key(key);
            sorted
        }
    };
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
    let rows_left: Vec<usize> = {
        let key = |i: usize| rows[i].1.3.map(|g| g.node_id);
        let mut out = vec![1usize; rows.len()];
        let mut end = rows.len();
        for i in (0..rows.len()).rev() {
            if i + 1 < rows.len() && key(i + 1) != key(i) {
                end = i + 1;
            }
            out[i] = end - i;
        }
        out
    };
    // Ширина таблицы — сумма ОБЪЕДИНЕНИЙ, а не число ячеек: строка из двух
    // ячеек с `colspan=2` даёт четыре колонки, и без этого содержимое
    // выталкивалось в неявные ряды.
    let cols = rows
        .iter()
        .map(|(r, _)| {
            r.children
                .iter()
                .filter_map(|c| match c {
                    Node::Element(e) if is_cell(e) => Some(
                        e.attr("colspan")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1),
                    ),
                    _ => None,
                })
                .sum::<usize>()
        })
        .max()
        .unwrap_or(1)
        .max(1) as u16;

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
    let mut under: Vec<AnyElement> = vec![];
    // Фоны ячеек сросшейся модели — отдельным слоем под кромками
    // (`interact::CellBgs`): так кромки красятся поверх фонов ячеек, но под
    // их содержимым — у Blink сросшиеся кромки идут в фазе
    // `kDescendantBlockBackgroundsOnly` (`box_fragment_painter.cc:952`), а
    // строчное/плавающее/позиционированное содержимое ячеек — позже
    // (`collapsed-border-paint-phase-001`, `collapsed-borders-painting-order-
    // 009/010/012/013`: вложенный стол и инлайн-блок с отрицательным полем
    // накрывались кромками внешнего стола).
    let cell_bgs: crate::interact::CellBgs = Default::default();
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
    let zero_cols: Vec<bool> = {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        // Ширина стола ДОЛЕЙ решается от содержащего блока: `width: 80%` в
        // шестистах сорока — это 512 (`fixed-table-layout-023`).
        let своя_ширина = match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        };
        match (e.style.table_fixed == Some(true), своя_ширина) {
            (true, Some(tw)) => {
                let side = |l: Option<Len>| px_of(l).unwrap_or(0.0);
                let tbz = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                let base = tw - side(tbz.left) - side(tbz.right) - gap * (f32::from(cols) + 1.0);
                let mut declared = vec![None::<f32>; cols as usize];
                for (i, w) in from_cols.iter().enumerate() {
                    if let (Some(w), Some(slot)) = (w, declared.get_mut(i)) {
                        *slot = Some(*w);
                    }
                }
                if let Some(row) = row_elements.first() {
                    let mut i = 0usize;
                    for c in &row.children {
                        let Node::Element(cell) = c else { continue };
                        if !is_cell(cell) {
                            continue;
                        }
                        let span = cell
                            .attr("colspan")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1);
                        // Доля ячейки первого ряда — тоже ЗАЯВЛЕННАЯ
                        // ширина: `width: 50%` при столе в сто точках это
                        // пятьдесят, и вместе со своим отступом дорожка
                        // забирает всё место (`fixed-table-layout-025`).
                        let своя = match cell.style.width {
                            Some(Len::Px(v)) => Some(v),
                            Some(Len::Pct(k)) => Some(base.max(0.0) * k),
                            _ => None,
                        };
                        if span == 1
                            && let Some(w) = своя
                            && let Some(slot) = declared.get_mut(i)
                            && slot.is_none()
                        {
                            let b = cell.style.borders();
                            *slot = Some(
                                w + side(cell.style.padding.left)
                                    + side(cell.style.padding.right)
                                    + side(b.left)
                                    + side(b.right),
                            );
                        }
                        i += span;
                    }
                }
                let sum: f32 = declared.iter().flatten().sum();
                let свободно = base - sum;
                (0..cols as usize)
                    .map(|i| declared.get(i).copied().flatten().is_none() && свободно <= 0.5)
                    .collect()
            }
            _ => vec![false; cols as usize],
        }
    };
    let mut busy: Vec<u16> = vec![0; cols as usize];
    let (win_edges, outer_win) = table_border_widths::resolve(
        e, &row_elements, &rows.iter().map(|(_, carry)| carry.3).collect::<Vec<_>>(),
        &rows_left, cols, table_font, &table_family,
    );
    // Вертикальность САМОЙ таблицы: `inherited` внутри цикла рядов
    // перекрыт слоем группы строк (`<tbody>` с письмом травил гейты,
    // table-progression-htb-001 — письмо к рядам и группам НЕ применяется).
    let table_is_vertical = e.style.vertical == Some(true) || inherited.vertical == Some(true);
    for (ri, row) in row_elements.iter().enumerate() {
        let mut ix = 0usize;
        for slot in busy.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        for c in &row.children {
            let Node::Element(cell) = c else { continue };
            if !is_cell(cell) {
                continue;
            }
            while ix < busy.len() && busy[ix] > 0 {
                ix += 1;
            }
            let span = cell
                .attr("colspan")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            // Занятость колонок — по УРЕЗАННОМУ охвату (см. `rows_left`).
            let rspan = row_span_in_group(cell, rows_left.get(ri).copied().unwrap_or(1))
                .min(usize::from(u16::MAX)) as u16;
            for c2 in ix..(ix + span).min(busy.len()) {
                busy[c2] = rspan;
            }
            if span == 1 && ix < col_widths.len() {
                // Дорожку задаёт размер ячейки вдоль ИНЛАЙН-ОСИ ТАБЛИЦЫ.
                // Вертикальная таблица: ось вертикальна — дорожка из ВЫСОТЫ
                // ячейки (width остаётся её коробке, table-cell-align-002).
                // Горизонтальная: из ширины; у ортогональной ячейки
                // (вертикальное письмо в htb-таблице) `block-size` лёг в
                // height (размеры при вертикали не переставляются, см.
                // resolve_logical) — он и задаёт колонку.
                let table_vertical =
                    e.style.vertical == Some(true) || inherited.vertical == Some(true);
                let orthogonal = cell.style.vertical == Some(true) && !table_vertical;
                let source = if table_vertical || orthogonal {
                    // У ортогональной ячейки width несёт ЛОГИЧЕСКИЙ
                    // inline-size — физически это ВЫСОТА, не колонка
                    // (table-cell-align-005/006); колонку задаёт block-size,
                    // осевший в height.
                    cell.style.height
                } else {
                    cell.style.width
                };
                // ЗАМЕРЕНО: CSS2 5117 -> 5128, oldfront 2352 -> 2340. Двенадцать
                // потерянных — семья `css-writing-modes/table-progression-*`:
                // её эталон горизонтальный, а тест вертикальный, и слагаемые
                // ложатся по разным осям. Пробовали отсекать вертикальное
                // письмо (2339) и считать добавку только горизонтальной (2338)
                // — обе хуже. Возвращаться вместе с вертикальной табличной
                // раскладкой.
                //
                // Дорожка = `width` ячейки ПЛЮС её горизонтальные отступы и
                // рамки (§17.5.2.1, коробка содержимого); в сросшейся модели
                // рамка входит половиной. Та же формула стоит в ветке первого
                // ряда ниже; без неё колонка выходила у́же ячейки на её рамку
                // (`margin-applies-to-001..007`).
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(p)) => p,
                    _ => 0.0,
                };
                let extra = if cell.style.border_box == Some(true) {
                    0.0
                } else {
                    let b = cell.style.borders();
                    let border = if table_vertical || orthogonal {
                        side(b.top) + side(b.bottom)
                    } else {
                        side(b.left) + side(b.right)
                    };
                    let pad = if table_vertical || orthogonal {
                        side(cell.style.padding.top) + side(cell.style.padding.bottom)
                    } else {
                        side(cell.style.padding.left) + side(cell.style.padding.right)
                    };
                    // В сросшейся модели дорожка считает ПОЛОВИНУ победившей
                    // линии — ту же, что легла в паддинг ячейки; своя кромка
                    // могла быть у́же соседней.
                    pad + if e.style.border_collapse == Some(true) {
                        let w = win_edges.get(&cell.node_id).copied().unwrap_or([
                            side(cell.style.borders().top),
                            side(cell.style.borders().right),
                            side(cell.style.borders().bottom),
                            side(cell.style.borders().left),
                        ]);
                        if table_vertical || orthogonal {
                            (w[0] + w[2]) / 2.0
                        } else {
                            (w[1] + w[3]) / 2.0
                        }
                    } else {
                        border
                    }
                };
                match source {
                    Some(Len::Px(v)) => {
                        let v = v + extra;
                        let slot = &mut col_widths[ix].0;
                        *slot = Some(slot.map_or(v, |old| old.max(v)));
                    }
                    Some(Len::Pct(k)) => {
                        let slot = &mut col_widths[ix].1;
                        *slot = Some(slot.map_or(k, |old| old.max(k)));
                    }
                    // Шрифтовые единицы решаются кеглем САМОЙ ячейки
                    // (наследование row -> table): `td { width: 2em }` при
                    // `table { font: 50px }` — колонка 100px, не пропуск.
                    Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) => {
                        let size = match cell
                            .style
                            .font_size
                            .or(row.style.font_size)
                            .or(inherited.font_size)
                        {
                            Some(Len::Px(v)) => v,
                            _ => table_font,
                        };
                        let family = cell
                            .style
                            .font_family
                            .clone()
                            .unwrap_or_else(|| table_family.clone());
                        let v = crate::metrics::spacing_px(Some(l), &family, size) + extra;
                        if v > 0.0 {
                            let slot = &mut col_widths[ix].0;
                            *slot = Some(slot.map_or(v, |old| old.max(v)));
                        }
                    }
                    _ => {}
                }
            }
            ix += span;
        }
    }
    // Слой ГРУПП КОЛОНОК и слой КОЛОНОК — две полосы, снизу вверх (§17.5.1:
    // «the next layer contains the column groups… on top of the column groups
    // are the areas representing the column boxes»). У каждой свой буфер
    // проб: площадь группы шире колоночной, и `background-position` у них
    // разный. Обе идут ПЕРЕД рядами: колонка рисуется ниже ряда
    // (css-tables-3 §layers).
    let grp_els = colgroup_elements(&e.children);
    let col_els = col_elements(&e.children);
    let mut grp_rects: Vec<Option<crate::interact::RowRects>> = vec![None; cols as usize];
    let mut col_rects: Vec<Option<crate::interact::RowRects>> = vec![None; cols as usize];
    let have_rows = !row_elements.is_empty();
    push_col_bands(
        &grp_els,
        opts.doc_salt,
        have_rows,
        &mut grp_rects,
        &mut under,
    );
    push_col_bands(
        &col_els,
        opts.doc_salt,
        have_rows,
        &mut col_rects,
        &mut under,
    );
    // Ширины рамки самой таблицы: крайние ячейки расползаются фоном на её
    // половину в сросшейся модели.
    // Кегль СВОЙ, а не жёсткие 16 точек: `border: 0.5em` у таблицы с крупным
    // шрифтом давал вчетверо тоньше линию (`border-conflict-element-001d/e`).
    let table_em = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let table_family = inherited.font_family.clone().unwrap_or_default();
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, &table_family, table_em);
    let table_border = e.style.borders();
    let bw = [
        px_of(table_border.top),
        px_of(table_border.right),
        px_of(table_border.bottom),
        px_of(table_border.left),
    ];
    let table_edges = crate::interact::cell_edges_for(e.node_id ^ opts.doc_salt);
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
    {
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
    let group_refs: std::collections::HashMap<u64, crate::interact::RefBox> =
        std::collections::HashMap::new();
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

pub(crate) fn is_cell(e: &Element) -> bool {
    table_roles::is_cell(e)
}

/// Есть ли в поддереве ячейки содержимое, которое красится ПОЗЖЕ сросшихся
/// кромок стола: строчный уровень (атомы строки, заменяемые, инлайн-столы —
/// фаза переднего плана), флоаты, позиционированные и контексты наложения
/// (CSS 2.1 прил. E, шаги 5-8; Blink `box_fragment_painter.cc:952-957`
/// красит кромки в `kDescendantBlockBackgroundsOnly`, то есть сразу после
/// фонов поточных блочных потомков). Блочный поточный потомок без этих
/// признаков остаётся под кромками — его в расчёт не берём, спускаясь в его
/// детей. Глубина ограничена: обход идёт у каждой ячейки.
pub(crate) fn cell_paints_over(nodes: &[Node], depth: u8) -> bool {
    depth > 0
        && nodes.iter().any(|n| match n {
            Node::Element(k) => {
                inline_level_box(k)
                    || k.style.float.unwrap_or(0) != 0
                    || matches!(
                        k.style.position,
                        Some(crate::computed::Position::Relative)
                            | Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                            | Some(crate::computed::Position::Sticky)
                    )
                    || stacking_context(&k.style)
                    || cell_paints_over(&k.children, depth - 1)
            }
            Node::Text(_) => false,
        })
}

/// Охват ячейки по рядам в пределах её группы: `left` — сколько рядов от
/// ряда ячейки до конца группы (включая его). HTML table model: охват за
/// конец группы урезается, `rowspan=0` тянется до конца группы; мусор и
/// отсутствие атрибута — один ряд.
pub(crate) fn row_span_in_group(cell: &Element, left: usize) -> usize {
    let left = left.max(1);
    match cell
        .attr("rowspan")
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        Some(0) => left,
        Some(n) => n.clamp(1, left),
        None => 1,
    }
}
