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
    let mut cells: Vec<AnyElement> = vec![];
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
    let mut cells_over: Vec<AnyElement> = vec![];
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
    let mut row_ix = 0i16;
    // Занятость колонок ячейками с rowspan из ПРЕДЫДУЩИХ рядов: без неё
    // номер колонки считался по порядку детей ряда и съезжал — рамки,
    // схлопнутые колонки и пробы фона приписывались не тем колонкам.
    // Алгоритм тот же, что у авторазмещения сетки: занятые клетки
    // пропускаются.
    let mut occupied: Vec<u16> = vec![0; cols as usize];
    let mut group_refs: std::collections::HashMap<u64, crate::interact::RefBox> =
        std::collections::HashMap::new();
    let tbl_style: &Computed = inherited;
    for (row, carry) in rows {
        row_ix += 1;
        let row_ref: crate::interact::RefBox = Default::default();
        for slot in occupied.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        // Фон ряда КАРТИНКОЙ (css-tables-3 §drawing-backgrounds): рисуется в
        // ЯЧЕЙКАХ, непрерывно от начала ряда, зазоры остаются чистыми.
        // Полоса на весь ряд несёт слой фона, но обрезает его прямоугольниками
        // ячеек, снятыми пробами прошлого кадра.
        let row_rects: Option<crate::interact::RowRects> = (row.style.bg_image.is_some()
            || row.style.gradient_raw.is_some()
            || !row.style.shadows.is_empty())
        .then(|| crate::interact::row_rects_for(row.node_id ^ opts.doc_salt));
        if let Some(rects) = &row_rects {
            // Градиент ряда идёт слоем-картинкой: источник понимает записи
            // `linear-gradient(...)` и растрирует их сам.
            let mut band_style = row.style.clone();
            if band_style.bg_image.is_none() {
                band_style.bg_image = band_style.gradient_raw.clone();
            }
            under.push(
                crate::interact::CellsClipped::new(rects.clone(), band_style).into_any_element(),
            );
        }
        // Фон ГРУППЫ рядов красится так же, как фон ряда (§17.5.1, слой 3):
        // полоса идёт от левого края крайней левой колонки до правого края
        // крайней правой и обрезается прямоугольниками ячеек. Своей коробки у
        // группы в сетке нет, поэтому картинка и градиент пропадали вовсе —
        // рисовался только сплошной цвет, который течёт вниз наследованием.
        // Обводка группы (`outline` — «Applies to: all elements», css-ui-4)
        // тоже идёт полосой: своей коробки у группы нет, а охват её ячеек
        // полоса уже считает (`visibility-collapse-border-spacing-001`:
        // `tbody { outline: 10px }` не рисовался вовсе).
        let grp_band: Option<crate::interact::RowRects> = carry.3.and_then(|g| {
            (g.style.bg_image.is_some()
                || g.style.gradient_raw.is_some()
                || !g.style.shadows.is_empty()
                || g.style.outline.is_some())
            .then(|| crate::interact::row_rects_for(g.node_id ^ opts.doc_salt))
        });
        if let (Some(rects), Some(g)) = (&grp_band, carry.3)
            && group_of
                .get(&row.node_id)
                .is_some_and(|(_, first, _)| *first)
        {
            let mut band_style = g.style.clone();
            if band_style.bg_image.is_none() {
                band_style.bg_image = band_style.gradient_raw.clone();
            }
            // Полоса ради одной обводки цвет не красит: сплошной цвет группы
            // без картинки несут ячейки (`collect_rows`), второй слой удвоил
            // бы полупрозрачный.
            if band_style.bg_image.is_none() && g.style.shadows.is_empty() {
                band_style.background = None;
            }
            under.push(
                crate::interact::CellsClipped::new(rects.clone(), band_style).into_any_element(),
            );
        }
        let shift = (carry.0, carry.1);
        // Письмо к строкам НЕ ПРИМЕНЯЕТСЯ (раскладку ряда ведёт таблица,
        // CSS Writing Modes §3.1) — но ВЫЧИСЛЕННОЕ значение наследуется в
        // ячейки как у любого свойства: `tr { writing-mode; line-height: 5ch }`
        // обязан дать ячейке вертикальное содержимое (ch-units-vrl-*).
        // Ряд у нас и так не строит своей коробки — урезать нечего.
        let own = row.style.clone();
        // Слой ГРУППЫ строк между таблицей и рядом: наследуемое с `<tbody>`
        // течёт вниз, как у любого предка.
        let group_layer;
        let inherited = match carry.3 {
            Some(g) => {
                group_layer = inline::inherit(inherited, &g.style);
                &group_layer
            }
            None => inherited,
        };
        // Направление на строке ОСТАЁТСЯ: замерено, что его обнуление сдвигает
        // ячейки в парах `position-relative-table-*-left` (29 → 25).
        // ПРОБОВАЛИ ТРИЖДЫ И ОТКАТИЛИ: доводить до ячеек наследуемые свойства
        // САМОЙ таблицы (`inline::inherit(inherited, &e.style)` как основа).
        // Дыра настоящая — `white-space` и шрифт с тега таблицы до ячейки не
        // доходят, — но цена: css-text −3 (`shaping-tatweel-002/003`,
        // `shaping-join-003`), а выигрыш НУЛЕВОЙ: семейство
        // `ws-break-spaces-applies-to` не двигается ни на пару. Значит
        // сохранённые пробелы в ячейке теряются НЕ здесь, и до того, как
        // найдено настоящее место, правка только вредит.
        // `inherited` — УЖЕ слитый стиль самой таблицы, поэтому второй мерж
        // сырого `e.style` разрешал относительные единицы повторно:
        // `font-size: 2em` на теге давал ячейке 64 точки вместо 32, строки
        // не влезали в колонку и таблица разъезжалась на лишние полосы
        // (вся семья `table-anonymous-objects-059…098`).
        let row_style = inline::inherit(inherited, &own);
        // Ряд БЕЗ ячеек с заданной высотой держит свою дорожку (CSS 2.1
        // §17.5.3: высота ряда — не меньше его `height`). Элементов сетки у
        // него нет, и дорожка пропадала: `border-collapse-empty-row` терял
        // 2/5/10 точек пустых рядов, и шаг рядов расходился с эталоном
        // (20+2 против 10+12). Заглушка на всю ширину ставится только когда
        // ряд не накрыт охватом сверху — иначе авторазмещение унесло бы её
        // в следующий ряд.
        if e.style.vertical != Some(true)
            && !row
                .children
                .iter()
                .any(|n| matches!(n, Node::Element(c) if is_cell(c)))
            && occupied.iter().all(|o| *o == 0)
            && let Some(Len::Px(h)) = row.style.height
            && h > 0.0
        {
            let mut ph = div().h(px(h)).col_span(cols);
            if e.style.vertical != Some(true) {
                ph = ph.col_start(1).row_start(row_ix);
            }
            cells.push(ph.into_any_element());
        }
        let mut col_ix = 0usize;
        for child in &row.children {
            let Node::Element(cell) = child else { continue };
            if !is_cell(cell) {
                continue;
            }
            while col_ix < occupied.len() && occupied[col_ix] > 0 {
                col_ix += 1;
            }
            // §17.6.1.1: `empty-cells: hide` прячет фон и рамку ПУСТОЙ
            // ячейки — в раздельной модели рамок. Пустая это та, у которой нет
            // ни текста, ни элементов-детей.
            let прячем_пустую = inline::inherit(&row_style, &cell.style).empty_cells_hide
                == Some(true)
                && e.style.border_collapse != Some(true)
                && {
                    let mut текст = String::new();
                    gather_text(&cell.children, &mut текст);
                    текст.trim().is_empty()
                        && !cell.children.iter().any(|n| matches!(n, Node::Element(_)))
                };
            // Ячейка в НУЛЕВОЙ дорожке: свои горизонтальные отступ и рамку
            // она держать не может — дорожки под них нет (§17.5.2.1).
            let cell = &if прячем_пустую {
                let mut copy = cell.clone();
                copy.style.background = None;
                copy.style.gradient = None;
                copy.style.bg_image = None;
                copy.style.border_visible = [Some(false); 4];
                copy.style.border_width = Default::default();
                copy
            } else {
                cell.clone()
            };
            let cell = &if zero_cols.get(col_ix).copied().unwrap_or(false) {
                let mut copy = cell.clone();
                copy.style.padding.left = Some(Len::Px(0.0));
                copy.style.padding.right = Some(Len::Px(0.0));
                copy.style.border_width.left = Some(Len::Px(0.0));
                copy.style.border_width.right = Some(Len::Px(0.0));
                copy
            } else {
                cell.clone()
            };
            let mut cm = inline::inherit(&row_style, &cell.style);
            pseudo_line_layers::install(cell, &mut cm);
            // `vertical-align` is not inherited (CSS 2.1 §10.8.1): only `td`/
            // `th` take their row's value, through the UA rule
            // `vertical-align: inherit` (HTML §15.3.9). A generic
            // `display: table-cell` box keeps its own value or the initial
            // `baseline` (`vertical-align-applies-to-*`: a row group's
            // `bottom` must not move the cell's content).
            if cell.style.vertical_align.is_none() && !html_cell(cell) {
                cm.vertical_align = None;
            }
            // Потолок вертикальной ячейки режет доступное место её
            // ортогонального потока — как у блока (см. ortho_limit в
            // element): стопка глифов переносится на следующую колонку по
            // нему (table-cell-002: td vertical-rl с max-height 100 —
            // зелёный квадрат из двух колонок).
            if cell.style.vertical == Some(true)
                && cm.ortho_limit.is_none()
                && let Some(Len::Px(h)) = cell.style.height.or(cell.style.max_height)
            {
                cm.ortho_limit = Some(h);
            }
            // Ячейка ПАРАЛЛЕЛЬНОЙ таблицы (письмо вертикально у самого стола,
            // ячейка его наследует): её инлайн-мера — дорожка КОЛОНКИ, а не
            // инлайн-размер всего стола. Предел, приехавший сверху
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
            if e.style.vertical == Some(true) && cm.ortho_limit.is_some() {
                cm.ortho_col = true;
            }
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
            // (замерено: `flexbox_rowspan-overflow` рисовал пустую страницу).
            // Коробка ячейки при этом обрезать содержимое не перестаёт.
            // Объединённая ячейка ЧЕРЕЗ схлопнутую колонку обрезается по
            // урезанной ширине (css-tables-3 §visibility-collapse-cell-
            // rendering): содержимое не расталкивает оставшиеся колонки.
            let spans_collapsed = span_cols > 1
                && (col_ix..col_ix + span_cols as usize)
                    .any(|i| cols_collapsed.get(i).copied().unwrap_or(false));
            let clipped = cell.style.overflow_x == Some(crate::computed::Overflow::Hidden)
                || cell.style.overflow_y == Some(crate::computed::Overflow::Hidden)
                || spans_collapsed;
            let mut cell = cell.clone();
            // `padding: inherit` и `border: inherit` решаются только в СЛИТОМ
            // стиле (`cm`, `inline.rs` ветки `padding_inherit`/`border_inherit*`),
            // а коробку ячейки, полкромки сросшейся модели и `cellpadding`
            // ниже строят из СЫРОГО стиля: там лежало умолчание `td {padding:
            // 1px}` вместо 5px ряда (CSS 2.1 §6.2.1 — значение родителя).
            // `row-margin-border-padding`, `row-group-margin-border-padding`:
            // все четыре стола с `.inherited` выходили меньше эталона.
            if cell.style.padding_inherit || cell.style.padding_inherit_side.contains(&true) {
                cell.style.padding = cm.padding;
            }
            if cell.style.border_inherit
                || cell.style.border_inherit_w.contains(&true)
                || cell.style.border_inherit_s.contains(&true)
                || cell.style.border_inherit_c.contains(&true)
            {
                cell.style.border_width = cm.border_width;
                cell.style.border_visible = cm.border_visible;
                cell.style.border_side_styles = cm.border_side_styles;
                cell.style.border_colors = cm.border_colors;
                cell.style.border_color = cm.border_color;
                cell.style.border_dashed = cm.border_dashed;
                cell.style.border_dotted = cm.border_dotted;
            }
            // ПРОБОВАЛИ И ОТКАТИЛИ: держать внутри ячейки ПОЛОВИНУ её кромки
            // прозрачной рамкой, а внутри таблицы — половину своей (§17.6.2:
            // «row-width = (0.5 * border-width0) + padding-left1 + …», «the
            // width of the table includes half the table border»), заодно сняв
            // поправку `shift` у проб. Проба по 39 парам семей
            // `table-backgrounds-b[cs]-*`, `collapsing-border-model-*`,
            // `border-conflict-style-10*`: флипов ноль, все шесть `bc-*`
            // подтянулись (13.73 -> 12.10, 5.00 -> 4.01, 1.00 -> 0.81), но
            // потеряны `fixed-table-layout-027` (0.00 -> «красное видно») и
            // `collapsing-border-model-008` (0.00 -> 1.36). Половина берётся от
            // ПОБЕДИВШЕЙ кромки соседей (§17.6.2.1), а не от своей: без
            // разрешения ширин по всей сетке модель не сходится.
            //
            // Сросшиеся рамки (border-collapse): рамки С ЯЧЕЕК СНИМАЮТСЯ
            // целиком — их рисует отдельный слой кромок на линиях сетки
            // (см. interact::EdgePainter): кромка соседей ОДНА, рисуется
            // поверх фонов, и «шире побеждает» решается наложением.
            let cell_edge = if collapse_cells {
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
                    crate::computed::Sides {
                        top: pick(own.top, merged.top),
                        right: pick(own.right, merged.right),
                        bottom: pick(own.bottom, merged.bottom),
                        left: pick(own.left, merged.left),
                    }
                };
                let widths = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                let black = crate::value::Color {
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
                let side_style = |i: usize| {
                    cell.style.border_side_styles[i].unwrap_or(if widths[i] > 0.0 { 9 } else { 0 })
                };
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
                cell.style.padding = crate::computed::Sides {
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
            } else {
                None
            };
            if clipped {
                cell.style.overflow_x = None;
                cell.style.overflow_y = None;
            }
            // Презентационный `cellpadding="N"` таблицы: хинт ниже авторского
            // padding, но выше умолчания браузера `td { padding: 1px }` —
            // применяется, только когда у ячейки ровно оно.
            if let Some(pad) = e
                .attr("cellpadding")
                .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            {
                let ua = |l: Option<Len>| matches!(l, Some(Len::Px(1.0)));
                let pd = &cell.style.padding;
                if ua(pd.top) && ua(pd.right) && ua(pd.bottom) && ua(pd.left) {
                    let v = Some(Len::Px(pad));
                    cell.style.padding = crate::computed::Sides {
                        top: v,
                        right: v,
                        bottom: v,
                        left: v,
                    };
                }
            }
            // Ячейка схлопнутой колонки не рисуется: колонка выброшена
            // (css-tables-3 §visibility-collapse-cell-rendering), её дорожка
            // нулевая, а краска ячейки торчала бы поверх соседей.
            if (col_ix..col_ix + span_cols as usize)
                .all(|i| cols_collapsed.get(i).copied().unwrap_or(false))
            {
                cell.style.hidden = Some(true);
            }
            table_spanning_size::preserve(
                &mut cell.style, &cm, span_cols, e.style.table_fixed == Some(true),
                table_is_vertical, spans_collapsed,
            );
            if matches!(cell.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))) {
                cell.style.width = None;
            }
            // Кегльная ширина уходит с коробки, как Px и доля: колонка
            // разрешает её сама (Em-ветка col_widths), а на коробке она
            // падала в запасной кегль 16px — ячейка `width: 2em` при шрифте
            // 50px сжималась до 32 точек, и прижим строк оставался без места
            // (table-cell-valign-003-ref). Ортогональную ячейку не трогаем:
            // её width — логический inline-size, он ниже перекладывается в
            // высоту.
            if !(cell.style.vertical == Some(true)
                && e.style.vertical != Some(true)
                && cell.style.width_from_inline)
                // У ВЕРТИКАЛЬНОЙ таблицы width остаётся коробке: дорожку
                // задаёт высота (table-cell-align-001/002).
                && !table_is_vertical
                && matches!(
                    cell.style.width,
                    Some(Len::Em(_)) | Some(Len::Ch(_)) | Some(Len::Ex(_))
                )
            {
                cell.style.width = None;
            }
            // У ВЕРТИКАЛЬНОЙ таблицы кегльная ширина ячейки остаётся на
            // коробке, но в точки её никто не переводил: `apply` добавляет
            // отступы только к `Len::Px`, и коробка выходила у́же дорожки на
            // свои отступы и рамки. Перекладываем в минимум, зеркально
            // правилу `height` -> `min_height` ниже.
            if table_is_vertical
                && !cell.style.width_from_inline
                && let Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) = cell.style.width
            {
                let size = match cell.style.font_size.or(row.style.font_size) {
                    Some(Len::Px(v)) => v,
                    _ => table_font,
                };
                let family = cell
                    .style
                    .font_family
                    .clone()
                    .unwrap_or_else(|| table_family.clone());
                let v = crate::metrics::spacing_px(Some(l), &family, size);
                if v > 0.0 {
                    cell.style.width = None;
                    cell.style.min_width = Some(Len::Px(v));
                }
            }
            // Письмо к рядам и группам рядов не применяется (css-writing-modes
            // §applies), а РАЗМЕЩЕНИЕ ячеек в решётке всегда ведёт письмо
            // таблицы — оно уже посчитано табличным кодом. Собственное письмо
            // ячейки остаётся: оно законно управляет её СОДЕРЖИМЫМ
            // (ортогональные ячейки, table-cell-align-002).
            // `ch` на высоте ячейки разрешается с письмом РЯДА: при
            // vertical + upright продвижение нуля — кегль (css-values-3,
            // ch-units-vrl-*). Только ch: полный resolve_em здесь двигал
            // em-высоты и был нетто-минусом (замерено, откат 8b59418-ядра).
            if let Some(Len::Ch(k)) = cell.style.height {
                // Флаги РЯДА, не свои: свой upright ячейки давал кегль там,
                // где эталон меряет лежачим нулём (ch-units-vrl-007/008 —
                // расхождение путей резолва div-эмуляции, вернуться при
                // унификации resolve_em).
                let upright = row.style.upright.or(inherited.upright) == Some(true);
                let vertical = row.style.vertical.or(inherited.vertical) == Some(true);
                let base = match inherited.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let ch = if vertical && upright {
                    base
                } else {
                    let family = inherited.font_family.clone().unwrap_or_default();
                    crate::metrics::ch_ex_px(&family, base).0
                };
                cell.style.height = Some(Len::Px(k * ch));
            }
            // Ортогональная ячейка (своё письмо вертикально, таблица
            // горизонтальна) живёт в НЕповёрнутой сетке: логический
            // inline-size осел в width (resolve_logical оси не переставляет —
            // подгонка под поворотную модель), но коробку ячейки никто не
            // вращает — её строчная ось физически ВЕРТИКАЛЬНА, и размер
            // обязан лечь высотой (table-cell-align-005/006).
            if cell.style.vertical == Some(true)
                && e.style.vertical != Some(true)
                && cell.style.height.is_none()
                && cell.style.width_from_inline
                && cell.style.width.is_some()
            {
                cell.style.height = cell.style.width.take();
            }
            // `em` на высоте ячейки — тем же точечным резолвом, что и `ch`:
            // без него логическая высота `inline-size: 2em` не проходила
            // Px-ветку ниже и min-height ряда не ставился.
            if let Some(Len::Em(k)) = cell.style.height {
                let base = match cell.style.font_size.or(inherited.font_size) {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                cell.style.height = Some(Len::Px(k * base));
            }
            // Предел ортогонального потока пересчитывается ПОСЛЕ переклада
            // inline-size в высоту: блок выше (у `let mut cm`) высоты ещё
            // не видел (table-cell-align-005/006).
            if cell.style.vertical == Some(true)
                && cm.ortho_limit.is_none()
                && let Some(Len::Px(h)) = cell.style.height
            {
                cm.ortho_limit = Some(h);
            }
            // Ортогональная ячейка (вертикальный контент от ряда) не уже
            // ТОЛЩИНЫ своей вертикальной строки — вклад стека в дорожку
            // сжимался до колонки в один глиф (ch-units-vrl-001: 19 вместо
            // line-height 100).
            if cell
                .style
                .vertical
                .or(row.style.vertical)
                .or(inherited.vertical)
                == Some(true)
                && cell.style.min_width.is_none()
                && cell.style.width.is_none()
            {
                let upright = row.style.upright.or(inherited.upright) == Some(true);
                let base = match inherited.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let lh_raw = cell
                    .style
                    .line_height
                    .or(row.style.line_height)
                    .or(inherited.line_height);
                let lh = match lh_raw {
                    Some(Len::Px(v)) => Some(v),
                    Some(Len::Em(k)) => Some(k * base),
                    Some(Len::Ch(k)) => Some(if upright {
                        k * base
                    } else {
                        let family = inherited.font_family.clone().unwrap_or_default();
                        k * crate::metrics::ch_ex_px(&family, base).0
                    }),
                    _ => None,
                };
                if let Some(w) = lh {
                    cell.style.min_width = Some(Len::Px(w));
                }
            }
            // Высота ячейки — МИНИМУМ (css-tables §3.6): содержимое выше
            // растит ячейку, а не режется. `height: 20px` с блоком в 300
            // прятал всё под обрезкой.
            if let Some(Len::Px(h)) = cell.style.height {
                // Процентная высота ПРЯМОГО ребёнка решается от ЗАДАННОЙ
                // высоты ячейки (CSS 2.1 §10.5): раскладка под нами при
                // auto-росте ячейки трактует долю как auto, и ребёнок с
                // overflow и height:100% раздувался содержимым вместо
                // прокрутки в заданных ста точках.
                for child in cell.children.iter_mut() {
                    if let Node::Element(el) = child
                        && let Some(Len::Pct(k)) = el.style.height
                    {
                        el.style.height = Some(Len::Px(h * k));
                    }
                    // Пороги той же долей — от той же заданной высоты ячейки
                    // (CSS 2.1 §10.7: доля `max-height`/`min-height` считается
                    // как у `height`). Нерешённая доля у нас отбрасывается, и
                    // заменяемый ребёнок шёл природным размером: `<canvas
                    // 200×200 max-height: 100%>` в ячейке высотой 100 давал
                    // 200×200 вместо 100×100
                    // (`percent-height-replaced-in-percent-cell-002`).
                    if let Node::Element(el) = child
                        && let Some(Len::Pct(k)) = el.style.max_height
                    {
                        el.style.max_height = Some(Len::Px(h * k));
                    }
                    if let Node::Element(el) = child
                        && let Some(Len::Pct(k)) = el.style.min_height
                    {
                        el.style.min_height = Some(Len::Px(h * k));
                    }
                }
                cell.style.height = None;
                let floor = match cell.style.min_height {
                    Some(Len::Px(v)) => v.max(h),
                    _ => h,
                };
                cell.style.min_height = Some(Len::Px(floor));
            } else {
                // Высота ячейки НЕ задана: доля ребёнка решается от высоты
                // ряда, а вклад ряда меряется БЕЗ доли (двухпроходная
                // раздача css-tables-3 §height-distribution). Однопроходное
                // приближение: якорь — собственный min-height ребёнка,
                // прокрутка держит содержимое внутри него.
                for child in cell.children.iter_mut() {
                    if let Node::Element(el) = child
                        && let Some(Len::Pct(k)) = el.style.height
                        && el
                            .style
                            .overflow_y
                            .is_some_and(|o| o != crate::computed::Overflow::Visible)
                        && let Some(Len::Px(m)) = el.style.min_height
                    {
                        el.style.height = Some(Len::Px(m * k));
                    }
                }
            }
            // ПРОБОВАЛИ И ОТКАТИЛИ (§17.5.2.1, ячейка обязана влезть в свою
            // дорожку при фиксированной раскладке): `border_box` + нулевой
            // `min_width` — потеряно 7 (`margin-bottom-applies-to-001..007`),
            // приобретено 0; один нулевой `min_width` — 0 и 0. Красное в
            // `fixed-table-layout-025..031` держит не минимум ячейки.
            // ПЕРЕПРОВЕРЕНО УЖЕ: сужение гейта до `table-layout: fixed` плюс
            // ячейка без своей ширины — те же семь потерь
            // (`margin-bottom-applies-to-001..007` 0.03 -> 3.80), флипов ноль.
            // Они тоже с фиксированной раскладкой; ячейка там без ширины, и
            // отличить их от `025..031` этим признаком нельзя.
            // ТРЕТЬЯ ПОПЫТКА, ЗАМЕРЕНА И ОТКАЧЕНА (01.09): не трогать размер
            // вовсе, а обрезать содержимое ячейки её коробкой
            // (`overflow_hidden` при `table-layout: fixed` и ячейке без своей
            // ширины). Срез из 81 пары `fixed-table-layout-*` +
            // `margin-bottom-applies-to-*`: зелёных 69 → 12. Обрезка режет
            // законно вылезающее содержимое — вся семья `003a..003f`
            // ушла 0.00 → 2.00, `017..020` 0.00 → 1.55.
            //
            // Потолок высоты к ячейке не применяется вовсе (браузеры
            // игнорируют max-height на ячейках): содержимое выше — растит.
            if matches!(cell.style.max_height, Some(Len::Px(_))) {
                cell.style.max_height = None;
            }
            let cell = &cell;
            // Коробке ячейки нужен СЛИТЫЙ стиль: у сырого `cell.style`
            // шрифтовые единицы не разрешены, и `apply` считает их от жёстких
            // 16 точек — `padding: 1em` при кегле 20 давало 16
            // (`table-height-algorithm-008a/b/c`). Берутся только те слоты,
            // где это безопасно: ширину и её минимум решает дорожка, и их
            // подмена уже мерилась отдельно.
            let box_style = {
                let mut c = cell.style.clone();
                let fixup = |own: Option<Len>, merged: Option<Len>| match own {
                    Some(Len::Px(_)) | None => own,
                    _ => merged,
                };
                c.padding = crate::computed::Sides {
                    top: fixup(c.padding.top, cm.padding.top),
                    right: fixup(c.padding.right, cm.padding.right),
                    bottom: fixup(c.padding.bottom, cm.padding.bottom),
                    left: fixup(c.padding.left, cm.padding.left),
                };
                c.height = fixup(c.height, cm.height);
                // Толщина рамки — тем же правилом: `border: 1em solid` у
                // ячейки доезжало сюда неразрешённым `Em`, а раскладка кладёт
                // только `Px` (`apply.rs`: прочие длины молча отбрасываются),
                // и рамка пропадала целиком (`table-height-algorithm-008b/c`
                // против зелёной `-008a`, где то же самое написано отступом).
                c.border_width = crate::computed::Sides {
                    top: fixup(c.border_width.top, cm.border_width.top),
                    right: fixup(c.border_width.right, cm.border_width.right),
                    bottom: fixup(c.border_width.bottom, cm.border_width.bottom),
                    left: fixup(c.border_width.left, cm.border_width.left),
                };
                c
            };
            // Сплошной цвет ячейки сросшейся модели уходит в слой под
            // кромками (`cell_bgs`, см. объявление): коробка остаётся без
            // заливки, цвет пишет проба. Картинка, градиент, `background-clip`,
            // спрятанная или преобразованная ячейка красятся по-прежнему на
            // месте — для них слой не строится.
            let bg_layered = paint_layers
                && box_style.bg_clip.is_none()
                && box_style.gradient.is_none()
                && box_style.bg_image.is_none()
                && box_style.hidden != Some(true)
                && box_style.opacity.is_none_or(|o| o >= 1.0)
                && box_style.transform.is_none()
                && box_style.translate.is_none()
                && box_style.filter.is_none();
            let mut box_style = box_style;
            let own_bg = box_style.background;
            if bg_layered {
                box_style.background = None;
            }
            let mut d = styled_div_with(cell, &box_style);
            // Заливка строки И ГРУППЫ строк: своей коробки у них в общей сетке
            // не остаётся, поэтому фон рисуют ячейки. Раньше бралась только
            // строка, и `<tbody style="background">` пропадал молча
            // (`position-relative-table-tbody-left`).
            let mut layer_bg = bg_layered.then_some(own_bg).flatten();
            if let Some(bg) = carry.2 {
                // Ряд с КАРТИНКОЙ красит и цвет САМ (см. CellsClipped) —
                // ячейка его не дублирует, иначе цвет ложится поверх
                // картинки. Ряду только с тенью цвет оставляют ячейки.
                let picture = row.style.bg_image.is_some() || row.style.gradient_raw.is_some();
                if !picture {
                    if bg_layered {
                        layer_bg = Some(bg);
                    } else {
                        d = d.bg(bg.to_hsla());
                        // A row/group background is replicated here only for
                        // painting (CSS 2.1 section 17.5.1); it must not snap
                        // each cell's text into an independent fill frame.
                        d.style().css_synthetic_background = Some(true);
                    }
                }
            }
            if let Some(bg) = layer_bg {
                d = d.child(crate::interact::cell_bg_probe(cell_bgs.clone(), bg.to_hsla()));
            }
            // Сдвиг строки или её группы: собственного элемента у них нет,
            // поэтому край, заданный на `<tr>`/`<tbody>`, двигает ячейки.
            // A relatively positioned cell's own percentage insets resolve
            // against the row's specified height (`position-relative-013`),
            // not the table grid the cell is laid out in; the row offset adds.
            let own_pct = cell.style.position == Some(crate::computed::Position::Relative)
                && [cell.style.inset.left, cell.style.inset.right, cell.style.inset.top, cell.style.inset.bottom]
                    .iter()
                    .any(|l| matches!(l, Some(Len::Pct(_))));
            if own_pct {
                let own = relative_shift(cell, Some(row));
                d = d.relative().left(px(shift.0 + own.0)).top(px(shift.1 + own.1));
                let s = d.style();
                s.inset.right = None;
                s.inset.bottom = None;
            } else if shift != (0.0, 0.0) {
                d = d.relative().left(px(shift.0)).top(px(shift.1));
            }
            // Умолчание браузера для ячейки — `vertical-align: middle`: без
            // него полоса высотой 10px в строке 22px стояла на 6 точек выше.
            // Ортогональная ячейка заводит СВОЙ контекст форматирования и
            // раскладывает содержимое в СВОЁМ письме (css-writing-modes-4
            // §3.1: `writing-mode` применяется к `table-cell`; §7.1: правила
            // горизонтальной оси переходят на вертикальную). Значит ось
            // блочного потока внутри неё ГОРИЗОНТАЛЬНА: дети идут справа
            // налево у `vertical-rl`/`sideways-rl` и слева направо у
            // `vertical-lr`/`sideways-lr`, а не столбиком. Тот же приём, что
            // у голого блока в `element()` (:14320). Blink строит место
            // ячейки её собственным письмом —
            // `table_layout_utils.cc:218`:
            // `ConstraintSpaceBuilder(table_writing_direction.GetWritingMode(),
            //                         cell_writing_direction, /* is_new_fc */ true)`.
            let ortho_cell = cm.vertical == Some(true) && e.style.vertical != Some(true);
            let mut d = if ortho_cell {
                let d = d.flex();
                if cm.vertical_rl == Some(true) {
                    d.flex_row_reverse()
                } else {
                    d.flex_row()
                }
            } else {
                d.flex().flex_col()
            };
            if ortho_cell {
                // ОРТОГОНАЛЬНАЯ ячейка (вертикальный контент в горизонтальной
                // таблице): строчная ось вертикальна — `text-align` правит
                // ВЕРТИКАЛЬНОЕ положение строки (line-left = верх), а
                // `vertical-align` уходит на поперечную ось
                // (table-cell-align-005/006).
                use crate::computed::TextAlign;
                // `start`/`end` — края СТРОКИ: вертикальная строка идёт
                // сверху вниз, `dir=rtl` разворачивает её снизу вверх.
                let rtl = cm.rtl == Some(true);
                d = match cm.text_align {
                    Some(TextAlign::Right) => d.items_end(),
                    Some(TextAlign::Center) => d.items_center(),
                    Some(TextAlign::End) if !rtl => d.items_end(),
                    Some(TextAlign::Start) if rtl => d.items_end(),
                    _ => d.items_start(),
                };
                // Начальное значение `vertical-align` — `baseline` (§17.5.3),
                // и у одиночного ряда это ВЕРХ ячейки, а не середина. Пока
                // умолчанием стояла середина, содержимое опускалось на
                // полразницы высот (`direction-applies-to-005`: квадрат на
                // 30 точек ниже эталона).
                d = match cm.vertical_align {
                    Some(Align::End) => d.justify_end(),
                    Some(Align::Center) => d.justify_center(),
                    _ => d.justify_start(),
                };
            } else {
                d = match cm.vertical_align {
                    Some(Align::End) => d.justify_end(),
                    Some(Align::Center) => d.justify_center(),
                    _ => d.justify_start(),
                };
                // `vertical-align: baseline` (CSS 2.1 §17.5.3): первые
                // базовые ячеек ряда совпадают, ряд растёт на сдвиг, а
                // коробка ячейки по-прежнему заполняет ряд — сдвигается
                // только содержимое (taffy `Style::table_cell_baseline`;
                // Blink `table_layout_utils.cc` `ComputeRowBaseline`).
                // Значение — СОБСТВЕННОЕ ячейки: `vertical-align` не
                // наследуется, а слитый `cm` тянет его от любого предка.
                // Только `td`/`th` берут значение ряда (UA-правило
                // `td, th { vertical-align: inherit }`).
                let own_va = cell.style.vertical_align.or(
                    if html_cell(cell) {
                        row.style.vertical_align
                    } else {
                        None
                    },
                );
                if own_va == Some(Align::Baseline) && e.style.vertical != Some(true) {
                    d.style().table_cell_baseline = Some(true);
                }
            }
            // Вертикальное письмо таблицы: ряды идут ПОПЕРЁК — охваты
            // меняются осями вместе с сеткой (css-writing-modes-3 §8).
            let (grid_cols, grid_rows) = if e.style.vertical == Some(true) {
                (span_rows as u16, span_cols as u16)
            } else {
                (span_cols, span_rows as u16)
            };
            if grid_cols > 1 {
                d = d.col_span(grid_cols);
            }
            if grid_rows > 1 {
                d = d.row_span(grid_rows);
            }
            // Явные координаты вместо авто-потока: у `vertical-rl` ряды идут
            // от ПРАВОГО края, а авто-поток умеет только вперёд — реверс
            // рядов ломал охваты (замерено: -001 1.02 → 1.31, откачено).
            if e.style.vertical == Some(true) {
                let n_rows = row_elements.len() as i16;
                let gc = if e.style.vertical_rl == Some(true) {
                    n_rows - row_ix - (span_rows as i16) + 2
                } else {
                    row_ix
                };
                // Строчная ось вертикальной таблицы: `dir=rtl` разворачивает
                // её (ячейки снизу вверх), `text-orientation: upright`
                // ФОРСИРУЕТ ltr (§5.1 — upright задаёт направление ltr), а у
                // `sideways-lr` базовое направление само снизу вверх —
                // разворот инвертируется.
                let rtl_line = e.style.rtl == Some(true) && e.style.upright != Some(true);
                let base_up = e.style.sideways == Some(true) && e.style.vertical_rl != Some(true);
                let gr = if rtl_line != base_up {
                    cols as i16 - col_ix as i16 - span_cols as i16 + 1
                } else {
                    col_ix as i16 + 1
                };
                d = d.col_start(gc.max(1)).row_start(gr.max(1));
            } else if e.style.rtl == Some(true) {
                // `dir=rtl` на таблице: колонки идут от ПРАВОГО края
                // (CSS 2.2 §17.2) — та же явная расстановка, зеркалом.
                let gc = cols as i16 - col_ix as i16 - span_cols as i16 + 1;
                d = d.col_start(gc.max(1)).row_start(row_ix);
            } else {
                // Явная расстановка ВСЕГДА (CSS 2.1 §17.5.1: ячейка стоит в
                // ряду своего `<tr>` и в колонке по счёту с учётом охватов).
                // Авто-поток сетки рядов не знает: у ряда КОРОЧЕ прочих (одна
                // ячейка в столе из двух колонок) следующий ряд продолжал
                // заполнять ту же дорожку, и стол из `<thead>` «head» /
                // «body one» / «body two» / «foot» выходил «head body / one
                // body / two foot» (`rules-groups`, снимок s1234 против
                // эталона с явной расстановкой). Заодно порядок детей сетки
                // свободен для слоёв краски (см. `cells_over`).
                d = d.col_start(col_ix as i16 + 1).row_start(row_ix);
            }
            for c in col_ix..(col_ix + span_cols as usize).min(occupied.len()) {
                occupied[c] = span_rows;
            }
            col_ix += span_cols as usize;
            // CSS 2.1 §9.4.1: a table cell establishes a block formatting
            // context, so its auto height contains its floats (§10.6.7). A
            // `td`/`th` gets its role from the tag and carries no `display`,
            // which `own_context_style` checks; without `CELL_BFC` its float
            // host took in-flow height only (`floats-wrap-bfc-001-right-
            // overflow`: the cell ended under the float's first 50px).
            CELL_BFC.with(|c| c.set(true));
            let inside = blocks(&cell.children, &cm, opts);
            CELL_BFC.with(|c| c.set(false));
            // Обрезанная ячейка не расталкивает колонки: её минимальный
            // вклад в дорожки НУЛЕВОЙ (css-sizing: automatic minimum при
            // overflow, отличном от visible, равен нулю) — иначе длинное
            // слово в обрезаемой объединённой ячейке раздавало ширину
            // колонкам, которых оно не должно касаться.
            if clipped {
                d = d.min_w(px(0.0));
            }
            let inside: Vec<AnyElement> = if spans_collapsed {
                // Ячейка через схлопнутую колонку: содержимое НЕ влияет на
                // ширины колонок вовсе (css-tables-3 §visibility-collapse) —
                // раскладка не должна его мерить, поэтому слой абсолютный.
                vec![
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .overflow_hidden()
                        .children(inside)
                        .into_any_element(),
                ]
            } else if clipped {
                vec![table_clipped_content::wrap(&mut d, inside)]
            } else if matches!(cm.vertical_align, Some(Align::Center) | Some(Align::End))
                && e.style.vertical != Some(true)
                && cell.children.iter().any(|n| {
                    matches!(n, Node::Element(c) if matches!(
                        c.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    ) && matches!(c.style.inset.top, None | Some(Len::Auto))
                        && matches!(c.style.inset.bottom, None | Some(Len::Auto)))
                })
            {
                // CSS 2.1 §17.5.3 aligns the cell's IN-FLOW content; an
                // absolutely positioned child keeps the static position it
                // would have in that flow (§10.6.4). The cell aligns by flex
                // justification, which would centre the abspos box itself
                // (Flexbox §4.1) — align a wrapper of the contents instead,
                // whose height is the in-flow height only
                // (position-relative-table-*-left-absolute-child: HTML's
                // `vertical-align: middle` row groups lifted the box by half
                // its height).
                vec![div().w_full().flex().flex_col().children(inside).into_any_element()]
            } else {
                inside
            };
            let mut d = d;
            // Полосы фонов рядов и колонок в сросшейся модели начинаются от
            // СЕРЕДИНЫ рамки таблицы (CSS 2.1 §17.6.2): пробы сдвинуты на
            // полкромки — сами ячейки остаются в потоке с полной рамкой.
            // Полкромки таблицы лежит в её паддинге, полкромки ячейки — в
            // паддинге ячейки: полосы фонов встают по месту без поправки.
            let shift = (0.0, 0.0);
            if let Some((widths, colors, styles, doc_ix)) = cell_edge {
                d = d.child(crate::interact::edge_probe(
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
            if let Some(rects) = &row_rects {
                d = d.child(crate::interact::cell_rect_probe(
                    rects.clone(),
                    span_rows == 1,
                    shift,
                    cell_border,
                ));
            }
            if let Some(rects) = &grp_band {
                d = d.child(crate::interact::cell_rect_probe(
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
                ) {
                    if !probed.contains(&el.node_id) {
                        probed.push(el.node_id);
                        d = d.child(crate::interact::cell_rect_probe(
                            rects,
                            span_cols == 1,
                            shift,
                            cell_border,
                        ));
                    }
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
                ) {
                    if !probed_group.contains(&el.node_id) {
                        probed_group.push(el.node_id);
                        d = d.child(crate::interact::cell_rect_probe(
                            rects,
                            span_cols == 1,
                            shift,
                            cell_border,
                        ));
                    }
                }
            }
            // Кромки РЯДА (border на <tr>) — участник разбора сросшихся
            // конфликтов (CSS 2.1 §17.6.2.1: ячейка > ряд > группа >
            // колонка > таблица); в раздельной модели рамки ряда не
            // действуют вовсе (§17.6.1) — сюда попадает только collapse.
            if collapse_cells {
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
                    let black = crate::value::Color {
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
                        row.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 {
                            9
                        } else {
                            0
                        })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        4,
                        row.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки ГРУППЫ РЯДОВ: верх у первого ряда группы, низ у
            // последнего; `rules=groups` даёт тонкую сплошную по умолчанию.
            if collapse_cells && let Some((g, first, last)) = group_of.get(&row.node_id).copied() {
                let b = g.style.borders();
                let default_w = if rules_groups { 1.0 } else { 0.0 };
                let explicit_top =
                    g.style.border_width.top.is_some() || g.style.border_visible[0].is_some();
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
                    let black = crate::value::Color {
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
                    let side_style = |k: usize| {
                        g.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        3,
                        g.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки КОЛОНКИ (рамка <col>/<colgroup>) — участники разбора
            // сросшихся конфликтов (источник между ячейкой и таблицей):
            // ячейка колонки несёт её кромку на совпадающем со спаном
            // колонки краю; верх/низ — только крайние ряды.
            if collapse_cells {
                for i in cell_cols.clone() {
                    let Some(el) = col_els.get(i).copied().flatten() else {
                        continue;
                    };
                    let b = el.style.borders();
                    let cw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                    let hidden = el.style.border_side_styles.contains(&Some(1));
                    if !(cw.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let same = |j: i64| -> bool {
                        j >= 0
                            && col_els
                                .get(j as usize)
                                .copied()
                                .flatten()
                                .is_some_and(|o| o.node_id == el.node_id)
                    };
                    let left_edge = !same(i as i64 - 1);
                    let right_edge = !same(i as i64 + 1);
                    let last_row = row_ix as usize >= row_elements.len();
                    let widths = [
                        if row_ix == 1 { cw[0] } else { 0.0 },
                        if right_edge { cw[1] } else { 0.0 },
                        if last_row { cw[2] } else { 0.0 },
                        if left_edge { cw[3] } else { 0.0 },
                    ];
                    if !(widths.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let black = crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    let side_colour = |k: usize| {
                        el.style.border_colors[k]
                            .or(el.style.border_color)
                            .or(el.style.color)
                            .or(inherited.color)
                            .unwrap_or(black)
                    };
                    let colors = [
                        side_colour(0),
                        side_colour(1),
                        side_colour(2),
                        side_colour(3),
                    ];
                    let side_style = |k: usize| {
                        el.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 {
                            9
                        } else {
                            0
                        })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        2,
                        el.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки ГРУППЫ КОЛОНОК — свой источник, слабее колонки и сильнее
            // таблицы (§17.6.2.1 п.4). Без него `<colgroup style="border">` с
            // колонками внутри терял рамку целиком: `col_elements` отдаёт
            // внутренние колонки, а сама группа в разбор не попадала.
            if collapse_cells {
                for i in cell_cols {
                    let Some(el) = grp_els.get(i).copied().flatten() else {
                        continue;
                    };
                    let b = el.style.borders();
                    let cw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                    let hidden = el.style.border_side_styles.contains(&Some(1));
                    if !(cw.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let same = |j: i64| -> bool {
                        j >= 0
                            && grp_els
                                .get(j as usize)
                                .copied()
                                .flatten()
                                .is_some_and(|o| o.node_id == el.node_id)
                    };
                    let left_edge = !same(i as i64 - 1);
                    let right_edge = !same(i as i64 + 1);
                    let last_row = row_ix as usize >= row_elements.len();
                    let widths = [
                        if row_ix == 1 { cw[0] } else { 0.0 },
                        if right_edge { cw[1] } else { 0.0 },
                        if last_row { cw[2] } else { 0.0 },
                        if left_edge { cw[3] } else { 0.0 },
                    ];
                    if !(widths.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let black = crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    let side_colour = |k: usize| {
                        el.style.border_colors[k]
                            .or(el.style.border_color)
                            .or(el.style.color)
                            .or(inherited.color)
                            .unwrap_or(black)
                    };
                    let colors = [
                        side_colour(0),
                        side_colour(1),
                        side_colour(2),
                        side_colour(3),
                    ];
                    let side_style = |k: usize| {
                        el.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 {
                            9
                        } else {
                            0
                        })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        1,
                        el.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // `transform` ячейки, ряда и группы рядов (css-transforms-1
            // §transformable-element: «table-row-group, table-header-group,
            // table-footer-group, table-row, table-column-group,
            // table-column, table-cell»). Своей коробки у ряда и группы в
            // сетке нет, поэтому их ПЕРЕНОС (не зависящий от точки отсчёта)
            // переходит на каждую ячейку; поворот/масштаб ряда требует его
            // коробки и пока не применяется.
            let mut built = d.children(inside).into_any_element();
            // Also without own transform: a `preserve-3d` cell or one under a
            // 3D row needs its wrapper for the context chain.
            built = transformed(built, &cell.style, &row_style);
            let pure_shift = |t: &crate::computed::Transform| {
                !t.has_3d
                    && t.lin == [[1.0, 0.0], [0.0, 1.0]]
                    && t.tr[0][1] == 0.0
                    && t.tr[0][2] == 0.0
                    && t.tr[1][1] == 0.0
                    && t.tr[1][2] == 0.0
            };
            // Any other row/group transform resolves its origin and
            // percentages against the row/group box: the union of its cells'
            // boxes, shared by their wrappers (`interact::Transformed::ref_box`;
            // `transform-transformed-tr-contains-fixed-position`: `rotate(45deg)`
            // with `transform-origin: left` on the `<tr>`).
            // The wrappers also carry a `preserve-3d` chain through the
            // row and group (css-transforms-2 §3d-rendering-context:
            // `transform-table-009/011`); without transform, perspective or
            // 3D context `transformed_with` returns the cell unchanged.
            let rb = row
                .style
                .transform
                .as_ref()
                .is_some_and(|t| !pure_shift(t))
                .then(|| row_ref.clone());
            built = transformed_with(built, &row.style, inherited, rb);
            if let Some(g) = carry.3 {
                let rb = g
                    .style
                    .transform
                    .as_ref()
                    .is_some_and(|t| !pure_shift(t))
                    .then(|| group_refs.entry(g.node_id).or_default().clone());
                built = transformed_with(built, &g.style, tbl_style, rb);
            }
            if paint_layers && cell_paints_over(&cell.children, 24) {
                cells_over.push(built);
            } else {
                cells.push(built);
            }
        }
    }

    // Порядок слоёв сетки: полосы дорожек/рядов → фоны ячеек → сросшиеся
    // кромки → коробки ячеек с содержимым (см. `under`, `cell_bgs`). Прежде
    // слой кромок шёл ПОСЛЕДНИМ и накрывал всё содержимое ячеек.
    let mut grid_children = under;
    if paint_layers {
        grid_children
            .push(crate::interact::CellBgPainter::new(cell_bgs.clone()).into_any_element());
    }
    grid_children.extend(cells);
    if paint_layers {
        grid_children
            .push(crate::interact::EdgePainter::new(table_edges.clone()).into_any_element());
    }
    grid_children.extend(cells_over);
    let cells = grid_children;
    // Заголовок таблицы живёт ВНЕ коробки таблицы (CSS 2.1 §17.4:
    // анонимная обёртка держит заголовок и коробку) — рамка и обрезка
    // таблицы его не трогают; `caption-side: bottom` ставит его под сетку.
    let mut caps_top: Vec<AnyElement> = Vec::new();
    let mut caps_bot: Vec<AnyElement> = Vec::new();
    for c in &e.children {
        if let Node::Element(cap) = c
            && (cap.tag == "caption" || cap.style.is_caption == Some(true))
        {
            let cm = inline::inherit(inherited, &cap.style);
            // Сторона — с самого заголовка, при пустоте — от таблицы
            // (наследование caption-side).
            let cap_side_bottom =
                cap.style.caption_bottom.or(e.style.caption_bottom) == Some(true);
            // CSS 2.1 §9.4.1: a table caption is a block container that
            // establishes a block formatting context, so its auto height
            // contains its floats (§10.6.7), like a cell (`CELL_BFC`).
            CELL_BFC.with(|c| c.set(true));
            let inside = blocks(&cap.children, &cm, opts);
            CELL_BFC.with(|c| c.set(false));
            let built = styled_div_with(cap, &cm)
                .flex()
                .flex_col()
                .children(inside)
                .into_any_element();
            // A caption is a transformable block box (css-transforms-1
            // §transformable-element); its `transform` was dropped
            // (`transform-transformed-caption-contains-fixed-position`).
            let built = transformed(built, &cap.style, inherited);
            // ВСЕ подписи, а не первая. Прежний `break` ронял вторую целиком:
            // у таблицы с верхней И нижней подписью рисовалась только верхняя
            // (`table-border-002/003/004`, снимок `table-border-004`: зелёное
            // теста обрывается на y = 253, то есть на 110 + 20 + 20 = 150
            // css-пикселях, а нижняя подпись в 250 пикселей не нарисована
            // ВООБЩЕ). CSS 2.1 §17.4 и css-tables-3 §terminology кладут в
            // обёртку ВСЕ подписи; Blink — двумя петлями по всем подписям
            // своей стороны (`table_layout_algorithm.cc:988` «Add all the top
            // captions», `:1584` «Add all the bottom captions»).
            if cap_side_bottom {
                caps_bot.push(built);
            } else {
                caps_top.push(built);
            }
        }
    }

    // Оси таблицы ЛОГИЧЕСКИЕ, как и у сетки: колонки идут вдоль строки. При
    // вертикальном письме строка идёт сверху вниз, и дорожки колонок
    // становятся физическими рядами. Своей ветки у таблицы не было, и её
    // сетка строилась физической — мимо уже переставленных осей.
    // Ширины колонок фиксированной раскладки — из ПЕРВОГО ряда
    // (CSS 2.1 §17.5.2.1): ячейка с шириной держит её, остальные делят
    // остаток поровну.
    let first_row_widths: Vec<Option<f32>> = row_elements
        .first()
        .map(|row| {
            let mut out = vec![];
            for c in &row.children {
                if let Node::Element(cell) = c
                    && is_cell(cell)
                {
                    let span = cell
                        .attr("colspan")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    // Колонка = width + горизонтальные паддинги и рамки
                    // ячейки (§17.5.2.1, content-box); в сросшейся модели
                    // рамка входит ПОЛОВИНОЙ.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(p)) => p,
                        _ => 0.0,
                    };
                    let extra = if cell.style.border_box == Some(true) {
                        0.0
                    } else {
                        let b = cell.style.borders();
                        let border = side(b.left) + side(b.right);
                        side(cell.style.padding.left)
                            + side(cell.style.padding.right)
                            + if e.style.border_collapse == Some(true) {
                                // Половина ПОБЕДИВШЕЙ линии — та же, что
                                // легла в паддинг ячейки (§17.6.2.1).
                                let w = win_edges
                                    .get(&cell.node_id)
                                    .map(|w| (w[1] + w[3]) / 2.0)
                                    .unwrap_or(border / 2.0);
                                w
                            } else {
                                border
                            }
                    };
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разрешать ширину ячейки в
                    // единицах ШРИФТА (`width: 1em` доезжает сюда
                    // неразрешённой — `resolve_em` живёт в наследовании, а
                    // ширины колонок считаются раньше). Срез из 10 пар
                    // `separated-border-model-*`: 8 зелёных до и после, ни
                    // один вердикт не сдвинулся. Одной этой половины мало:
                    // цель (`-004c/-004d`) требует ещё нижней грани ширины
                    // стола по сумме дорожек — возвращать вместе с ней.
                    match cell.style.width {
                        Some(Len::Px(v)) if span == 1 => out.push(Some(v + extra)),
                        // Ширина в единицах шрифта — кеглем САМОЙ ячейки
                        // (ряд → таблица): без неё колонка `width: 1em`
                        // уходила в безразмерные, и пол ширины стола ниже
                        // (§17.5.2.1) не складывался (`separated-border-model-004c`).
                        Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) if span == 1 => {
                            let size = match cell.style.font_size.or(row.style.font_size) {
                                Some(Len::Px(v)) => v,
                                _ => table_font,
                            };
                            let family = cell
                                .style
                                .font_family
                                .clone()
                                .unwrap_or_else(|| table_family.clone());
                            let v = crate::metrics::spacing_px(Some(l), &family, size);
                            out.push((v > 0.0).then_some(v + extra));
                        }
                        // Доля считается от места, отдаваемого дорожкам:
                        // ширина таблицы за вычетом зазоров (§17.5.2.1,
                        // «a percentage value ... of the table width»).
                        // Известна, только когда ширина таблицы в точках.
                        Some(Len::Pct(p)) if span == 1 => match e.style.width {
                            Some(Len::Px(tw)) => {
                                let gaps = spacing.0 * (f32::from(cols) + 1.0);
                                out.push(Some((tw - gaps).max(0.0) * p + extra));
                            }
                            _ => out.push(None),
                        },
                        _ => out.extend(std::iter::repeat_n(None, span)),
                    }
                }
            }
            out
        })
        .unwrap_or_default();
    // `<col>`-ширины старше ячеек первого ряда (§17.5.2.1) и действуют и в
    // авто-раскладке: колонка с шириной держит её (как ширина ячейки).
    for (i, w) in from_cols.iter().enumerate() {
        if let (Some(w), Some(slot)) = (w, col_widths.get_mut(i)) {
            slot.0 = Some(slot.0.map_or(*w, |old| old.max(*w)));
        }
    }
    let first_row_widths: Vec<Option<f32>> = (0..cols as usize)
        .map(|i| {
            from_cols
                .get(i)
                .copied()
                .flatten()
                .or_else(|| first_row_widths.get(i).copied().flatten())
        })
        .collect();
    let tracks = track_list_collapsed(
        cols,
        e.style.table_fixed == Some(true),
        &first_row_widths,
        &col_widths,
        &cols_collapsed,
        &cols_pct,
        // Место под КОЛОНКИ, а не вся ширина стола: §17.5.2.1 считает долю
        // от ширины таблицы БЕЗ её рамок и без зазоров между ячейками
        // (`fixed-table-layout-022` расписывает это прямо в тексте: 533 − 58
        // рамок − 75 зазоров = 400).
        match match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        } {
            Some(v) => {
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(w)) => w,
                    _ => 0.0,
                };
                let bs = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                Some(v - side(bs.left) - side(bs.right) - gap * (cols as f32 + 1.0))
            }
            _ => None,
        },
    );
    // Таблица ЗАДАННОЙ высоты раздаёт лишнее место рядам БЕЗ своей высоты
    // (CSS 2.1 §17.5.3): ряд с высотой (своей или ячеек) держит её, остальные
    // делят остаток. Без этого средний ряд решётки 64/auto/64 в таблице 224px
    // схлопывался по содержимому, и вся середина уезжала.
    // Считается и от min-height: минимум так же растягивает таблицу, и
    // остаток обязан достаться безвысотным рядам.
    let table_tall = matches!(e.style.height, Some(Len::Px(_)))
        || matches!(e.style.min_height, Some(Len::Px(_)))
        // Доля высоты, которая РАЗРЕШАЕТСЯ (содержащий блок определён), —
        // такая же заданная высота: css-tables-3 «a 'height' property with a
        // value other than auto … will eventually be distributed to the height
        // of the rows». Неразрешимая доля — это `auto` (CSS 2.1 §10.5), её не
        // берём. Растянутую раскладкой коробку (элемент гибкого контейнера или
        // сетки со своей долей) тоже не берём: там доля в taffy падает в `auto`,
        // а ряды-доли в неопределённой высоте выравниваются по самому высокому.
        // Эталоны `fr-unit-ref`, `display-grid-ref`: стол `height:100%` в
        // абсолюте 400×100 держал ряды по тексту вместо 30/70.
        || (matches!(e.style.height, Some(Len::Pct(_)))
            && inherited.cb_height_def
            && !inherited.stretched);
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v135, `scout-fonts-tables-2026-09.md`
            // план 2): строить дорожки рядов и без `table_tall`. Срез 8557 общих:
            // +3 (`table-as-item-cell-percentage-001/003/004`) при −18 —
            // `margin-applies-to-001…007` (0.00 → 1.00),
            // `margin-bottom-applies-to-001…007` (0.03 → 3.00),
            // `table-cell-overflow-explicit-height-001/002` (0.00 → 8.77),
            // `percentage-sizing-of-table-cell-children-004` («красное видно»),
            // `subpixel-table-cell-height-001`. Ряд без заданной высоты обязан
            // остаться авто-дорожкой ТОЛЬКО в контексте, где стол не растянут.
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
    let grid_box = if e.style.vertical == Some(true) {
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
    };
    // Сросшиеся рамки: у таблицы нет паддинга, а кромка между её рамкой и
    // краевыми ячейками одна — ячейки накрывают ВНУТРЕННЮЮ ПОЛОВИНУ рамки
    // (CSS 2.1 §17.6.2). Рамка при этом рисуется ПОВЕРХ фонов ячеек, как и
    // все сросшиеся кромки: обычная рамка коробки красится под детьми и
    // закрашивалась бы их фоном. Поэтому у самой коробки рамка снимается,
    // её место держит паддинг, сетка выезжает на его половину, а красит
    // рамку кольцевой квад ПОСЛЕ сетки.
    let collapse = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
    // Минимумы таблицы меряются ПОЛНОЙ коробкой с рамкой и паддингом
    // (css-tables-3 §computing-the-table-height, CSSWG #5336): пороги
    // пересчитываются в контентные, компенсацию вернёт общий слой.
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
    // §17.4: `position` и края — свойства ОБЁРТКИ таблицы, а не её сетки;
    // ширину сетки решает §17.5.2.2 (сжатие по содержимому). Пока коробка
    // одна, абсолютная таблица с ОБОИМИ краями инлайн-оси получала ширину от
    // краёв, и колонки расползались: сжатие у нас выражено только
    // `align_self`, а его у абсолютной коробки с двумя краями не спрашивают.
    let split_wrapper = matches!(
        inherited.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) && edge_set(inherited.inset.left)
        && edge_set(inherited.inset.right)
        && e.style.width.is_none();
    // Стиль СЕТКИ — без позиционирования и краёв: их заберёт обёртка.
    let grid_style = split_wrapper.then(|| {
        let mut c = inherited.clone();
        c.position = None;
        c.inset = Default::default();
        c.z_index = None;
        c
    });
    let inherited: &Computed = grid_style.as_ref().unwrap_or(inherited);
    let needs_clone =
        collapse
            || table_border_box
            || min_h != e.style.min_height
            || min_w != e.style.min_width
            || fixed_floor.is_some();
    let host_style;
    let mut outer = if needs_clone {
        let mut c = inherited.clone();
        if collapse {
            c.border_width = Default::default();
            c.border_visible = [None; 4];
            // §17.6.2: внутрь таблицы уходит ПОЛОВИНА её кромки. Паддингом,
            // а не рамкой: проба кромок — абсолютный ребёнок по паддинг-боксу,
            // и рамка утащила бы линию сетки внутрь на свою величину.
            c.padding = crate::computed::Sides {
                top: Some(Len::Px(outer_win[0] / 2.0)),
                right: Some(Len::Px(outer_win[1] / 2.0)),
                bottom: Some(Len::Px(outer_win[2] / 2.0)),
                left: Some(Len::Px(outer_win[3] / 2.0)),
            };
        }
        // Пол фиксированной раскладки (см. `fixed_floor`) — сама ширина
        // коробки: сетка внутри ровно такой ширины.
        if let Some(f) = fixed_floor {
            c.width = Some(Len::Px(f));
        }
        c.min_height = min_h;
        c.min_width = min_w;
        if table_border_box {
            c.border_box = Some(true);
            // Вертикальное письмо: `height` — это ИНЛАЙН-размер стола
            // (Blink: `ComputeTableInlineSize` читает `style.LogicalWidth()`,
            // а при `vertical-*` это физическое `height`), и наш конвейер УЖЕ
            // потратил её как КОНТЕНТНУЮ величину: предел ортогонального
            // потока сеется из `e.style.height` без вычета краёв (блок
            // `merged.ortho_limit` выше), и повёрнутый абзац рвёт строку
            // ровно по нему. Значит второй раз, коробкой, та же величина
            // обязана лечь по content-box, иначе краи вычитаются дважды и
            // стол выходит короче содержимого на свои рамки
            // (`row-progression-vrl-002`: 140.0 вместо 180.0 при неизменной
            // туши). Флаг `border_box` один на обе оси, поэтому его НЕ
            // снимаем — иначе content-box получила бы и `width`, то есть
            // БЛОЧНАЯ ось, где border-box верен; вместо этого краи инлайн-оси
            // добавляются к самой величине. Гейт тот же, что у
            // транспонирования решётки и у `spacing_phys`.
            if e.style.vertical == Some(true)
                && let Some(Len::Px(h)) = c.height
            {
                // Сросшийся стол несёт свои краи не рамкой, а паддингом в
                // половину победившей кромки (см. ветку `collapse` выше) —
                // берём ровно то, что легло в коробку.
                let inline_edges = if collapse {
                    (outer_win[0] + outer_win[2]) / 2.0
                } else {
                    bw[0] + bw[2] + pad_px[0] + pad_px[2]
                };
                c.height = Some(Len::Px(h + inline_edges));
            }
        }
        host_style = c;
        styled_div_with(e, &host_style).flex().flex_col()
    } else {
        styled_div_with(e, inherited).flex().flex_col()
    };
    // Таблица без заданной ширины СЖИМАЕТСЯ по содержимому, а не растягивается
    // на родителя (CSS 2.1 §17.5.2, shrink-to-fit). Пока она растягивалась,
    // две короткие колонки разъезжались к противоположным краям — видно на
    // `shaping-tatweel-002`, где одинаковые знаки стояли по краям окна.
    //
    // Заданный `align-self` приём отменяет: у элемента СЕТКИ эта ось —
    // блочная (css-align-3 §6.2, `taffy: grid/alignment.rs:145`), сжатие по
    // строчной оси ведёт `justify-self`, и прижим к началу здесь только
    // отбирал у стола высоту дорожки. Метку ставит
    // `dom::grid_table_items_keep_stretch`; авторский `align-self` она же и
    // пропускает вперёд.
    if e.style.width.is_none() && e.style.align_self.is_none() {
        outer.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    // Пол GRIDMIN (css-tables-3 §3.9): гибкая раскладка не ужимает стол по
    // главной оси ниже min-content его решётки — `vendor/taffy` `flexbox.rs`,
    // признак `item_is_table` (`table-as-item-auto-min-width`, `-wide-content`).
    outer.style().item_is_table = Some(true);
    // Table baselines come from rows, never the empty grid/caption shim.
    outer.style().baseline_unavailable = Some(!have_rows);
    // An enclosing `inline-block` takes no baseline from a table at any block
    // depth (CSS 2.1 §10.8.1 counts line boxes only; Blink
    // `block_layout_algorithm.cc` `PropagateBaselineFromBlockChild`: "table's
    // don't contribute any baselines"). The wrappers below carry the same mark.
    outer.style().no_inline_block_baseline = Some(true);
    // КОРНЕВОЙ стол (`<html display: table>`): родитель — блок стенда, где
    // `align-self` не работает, и стол растягивался на всё окно. Гибкая
    // обёртка возвращает сжатие по содержимому и центрирование `margin: auto`.
    let root_table = matches!(e.tag.as_str(), "html" | "body") && e.style.width.is_none();
    // `border-spacing` задан по ЛОГИЧЕСКИМ осям таблицы: первое значение —
    // зазор между КОЛОНКАМИ (инлайн-ось), второе — между РЯДАМИ (блочная
    // ось). css-writing-modes-3 §7.2 «Dimension Mapping» переносит их на
    // физические оси письмом ТАБЛИЦЫ: в вертикальном письме инлайн-ось
    // вертикальна, поэтому первое значение становится физическим
    // ВЕРТИКАЛЬНЫМ зазором, второе — горизонтальным. Решётка выше уже
    // транспонирована (`grid_box`: дорожки колонок легли в
    // `grid_template_rows`, ряды пошли колонками), а зазоры оставались
    // физическими — стол выходил перекошенным зеркально
    // (`border-spacing-vrl-002`: 160.0×70.0 вместо 100×100). Гейт — тот же
    // `e.style.vertical`, что и у решётки: иначе они разъедутся.
    // Стол БЕЗ рядов не несёт блочных зазоров, без рядов и колонок — и
    // строчных: пустой `<table>` у Blink/Gecko — только рамки. У нас
    // оставалась коробка 4×4 (умолчание 2px), и в
    // `caption-relative-positioning` между двумя подписями светила красная
    // полоса. Столы с рядами не меняются.
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
        outer = outer.child(crate::interact::grid_probe(table_edges.clone(), bw));
    }
    if collapse && (bw.iter().any(|w| *w > 0.0) || e.style.border_side_styles.contains(&Some(1))) {
        // Рамка самой таблицы — участник разбора конфликтов: её кромки
        // уходят в тот же слой (EdgePainter), линии — внутренние края
        // рамочного места, победившая кромка рисуется наружу.
        let black = crate::value::Color {
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
        outer = outer.child(crate::interact::edge_probe(
            table_edges.clone(),
            bw,
            colors,
            styles,
            0,
            e.node_id as u32,
            half,
        ));
    }
    let shrink_wrap = root_table
        || (e.style.width.is_none()
            // Заданная высота или её порог приходят от РАСКЛАДКИ родителя:
            // обёртка рвёт эту связь (★ ЗАМЕРЕНО: без отсечки
            // `min-height-table-2` 0.00 -> 19.24).
            && e.style.height.is_none()
            && e.style.min_height.is_none()
            && !inherited.stretched
            && e.style.flex_basis.is_none()
            && e.style.align_self.is_none()
            && e.style.grid_col.is_none()
            && e.style.grid_row.is_none());
    let mut outer = outer;
    // Сжатие по содержимому — `min(max-content, доступное)` (CSS 2.1
    // §17.5.2.2: «the used width is the greater of W and MIN» при W =
    // ширине контейнера, если таблица шире): в ряду-обёртке стол обязан
    // ужиматься. Блоку потока сжатие выключено (`flex_shrink = 0` в
    // `collapsed`), и стол с длинным текстом вылезал из узкого родителя
    // на всю max-content ширину. Пол GRIDMIN держит `item_is_table`.
    if shrink_wrap && caps_top.is_empty() && caps_bot.is_empty() && !split_wrapper {
        outer.style().flex_shrink = Some(1.0);
    }
    let outer = outer;
    // Обёртка «заголовок + коробка»: заголовок вне рамки и обрезки.
    let outer = if caps_top.is_empty() && caps_bot.is_empty() {
        outer.into_any_element()
    } else {
        // `caption-side: top/bottom` — стороны block-start/block-end стола
        // (css-writing-modes-4 §6, особое исключение для caption-side): в
        // вертикальном письме заголовок стоит СБОКУ — справа при `vertical-rl`,
        // слева при `vertical-lr` (`caption-side-vrl-002`).
        let vertical = e.style.vertical == Some(true);
        let mut wrap = if vertical {
            div().flex().flex_row()
        } else {
            div().flex().flex_col()
        };
        // Элемент гибкого контейнера у стола с подписями — ОБЁРТКА
        // (css-flexbox-1 §4: «the table wrapper box becomes the flex item, and
        // the order and align-self properties apply to it … the flex item's
        // final size is calculated … as if the distance between the table
        // wrapper box's edges and the table box's content edges were all part
        // of the table box's border+padding area»). Рост и сжатие уходят на
        // обёртку, стол внутри неё забирает остаток и растягивается по её
        // ширине — подписи и стол одной ширины. Основа остаётся на столе: в
        // колонке `wrap` его главная ось та же, что у контейнера-колонки
        // (`table-as-item-inflexible-in-column-2`). Прижим `FlexStart` —
        // только вне гибкого контейнера: там он даёт сжатие по содержимому
        // (§17.5.2), а в гибком контейнере отбирал растяжение
        // (`table-as-item-stretch-cross-size*`, `-flex-cross-size`).
        let mut outer = outer;
        if inherited.flex_item && !vertical {
            let s = outer.style();
            let grow = s.flex_grow.take();
            let shrink = s.flex_shrink.take();
            let own_align = s.align_self.take();
            s.flex_grow = Some(1.0);
            let w = wrap.style();
            w.flex_grow = grow;
            w.flex_shrink = shrink;
            w.align_self = if e.style.align_self.is_some() { own_align } else { None };
            // Основа в РЯДУ: главная ось контейнера — строчная ось обёртки,
            // и основа, оставленная на столе внутри колонки `wrap`, там не
            // действует (у колонки это поперечная ось). Переносится на
            // обёртку вместе с рамкой и отбивкой стола content-box —
            // css-flexbox-1 §4: «as if the distance between the table wrapper
            // box's edges and the table box's content edges were all part of
            // the table box's border+padding area»
            // (`table-as-item-inflexible-in-row-2`: `flex: 0 0 80px; border:
            // 10px solid` — стол выходил 20 точек вместо 100).
            if !matches!(inherited.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
                && let Some(Len::Px(b)) = e.style.flex_basis
            {
                s.flex_basis = None;
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let bd = e.style.borders();
                let edges = if e.style.border_box == Some(true) {
                    0.0
                } else {
                    side(bd.left)
                        + side(bd.right)
                        + side(e.style.padding.left)
                        + side(e.style.padding.right)
                };
                w.flex_basis = Some(gpui::Length::Definite(px(b + edges).into()));
            }
        } else {
            wrap.style().align_self = Some(gpui::AlignItems::FlexStart);
        }
        // Инлайн-размер ОБЁРТКИ — инлайн-размер САМОГО стола, а не наоборот.
        // CSS 2.1 §17.4: «The width of the table wrapper box is the border-edge
        // width of the table grid box inside it … Percentages on 'width' and
        // 'height' on the table are relative to the table wrapper box's
        // containing block, NOT the table wrapper box itself» (то же
        // css-tables-3 §fixup-algorithm). Доля коробки рядов опиралась на
        // обёртку, а обёртка — гибкий КОРЕНЬ копии фрагмента
        // (`flow.rs` `layout_as_root`), и taffy тянет по доступному месту
        // только БЛОЧНЫЙ корень (`vendor/taffy/src/compute/mod.rs:68`
        // `if style.is_block()`): стол с `inline-size: 100%` и подписью
        // схлопывался до ширины содержимого — 20 css у
        // `table-grid-paint-htb-ltr` (две рамки) и 34.4 у `table-row-paint-htb-ltr`
        // (один `border-spacing`). Blink держит один размер на стол и обёртку:
        // `table_layout_algorithm.cc:139` `available_size = {table_inline_size,
        // kIndefiniteSize}` и `:71` `builder.SetAvailableSize(available_size)`.
        // Берём только точки и долю: `em`/`ch` у обёртки считались бы по ЧУЖОМУ
        // шрифту (`apply::len_to_gpui` ветка запасных величин), а стол с
        // `width: auto` обязан остаться сжатым по содержимому (§17.5.2).
        match if vertical { e.style.height } else { e.style.width } {
            Some(Len::Px(v)) if vertical => wrap = wrap.h(px(v)),
            Some(Len::Px(v)) => wrap = wrap.w(px(v)),
            Some(Len::Pct(v)) if vertical => wrap = wrap.h(gpui::relative(v)),
            Some(Len::Pct(v)) => wrap = wrap.w(gpui::relative(v)),
            _ => {}
        }
        // В ряду второй ребёнок сжимался в ноль — каждому своя ширина.
        let own_width = |x: AnyElement| -> AnyElement {
            if vertical {
                div().flex_shrink_0().child(x).into_any_element()
            } else {
                x
            }
        };
        // Порядок обёртки — верхние подписи, коробка рядов, нижние подписи
        // (css-tables-3 §terminology; Blink `table_layout_algorithm.cc:988`
        // и `:1584`). Тот же порядок мерит `table_shape`.
        let mut cap_wrap: Vec<AnyElement> = Vec::new();
        for cap_el in caps_top {
            cap_wrap.push(own_width(cap_el));
        }
        // Стол не уже подписи (css-tables-3 §computing-the-table-width:
        // «the used min-width of a table is the greater of the resolved
        // min-width, CAPMIN, and GRIDMIN»). Обёртка сжата по содержимому,
        // значит она шириной с самую широкую из подписи и стола, и растяжка
        // стола по ней даёт ровно max(стол, CAPMIN) — но только когда CAPMIN
        // известен, то есть у каждой подписи ширина в точках. Подпись
        // текстом мерилась бы max-content, а не min-content, и распирала бы
        // колонки (`anonymous-table-box-width-001`: пустая подпись в 100
        // точек, рамка `border-bottom: 100px` у стола нулевой ширины).
        let caps_px = e.children.iter().all(|n| match n {
            Node::Element(c) if c.tag == "caption" || c.style.is_caption == Some(true) => {
                matches!(c.style.width, Some(Len::Px(_)))
            }
            _ => true,
        });
        let mut outer = outer;
        if !vertical && caps_px && e.style.width.is_none() && e.style.align_self.is_none() {
            outer.style().align_self = Some(gpui::AlignItems::Stretch);
        }
        cap_wrap.push(own_width(outer.into_any_element()));
        for cap_el in caps_bot {
            cap_wrap.push(own_width(cap_el));
        }
        // Заголовок ПЕРЕД коробкой по порядку детей = у начала оси: для vrl
        // начало блочной оси — правый край, а ряд идёт слева направо, значит
        // переворачивается ВЕСЬ порядок обёртки. Прежнее
        // `cap_first = caption_bottom == (vertical && vertical_rl)` — тот же
        // разворот, записанный для ОДНОЙ подписи.
        if vertical && e.style.vertical_rl == Some(true) {
            cap_wrap.reverse();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        wrap.children(cap_wrap).into_any_element()
    };
    // Вторая половина §17.4: сама обёртка. Гибкий ряд возвращает сетке сжатие
    // по содержимому — тот же приём, что у корневого стола ниже.
    if split_wrapper {
        let mut wrap = Computed::default();
        wrap.position = e.style.position;
        wrap.inset = e.style.inset;
        wrap.z_index = e.style.z_index;
        let mut wrap = crate::apply::apply(div(), &wrap).flex().flex_row();
        wrap.style().no_inline_block_baseline = Some(true);
        return wrap.child(outer).into_any_element();
    }
    // Стол с `width: auto` СЖИМАЕТСЯ по содержимому (§17.5.2): у нас это
    // делает гибкий ряд-обёртка. Приём `align_self: FlexStart` выше работает
    // только когда родитель — гибкая колонка нашей сборки; под `body` со
    // сброшенными полями путь другой, и стол растягивался во всю ширину
    // (`html-display-table`, `root-box-002`). Обёртка снимает зависимость от
    // родителя. Элемент гибкого контейнера, сетки и ячейки не заворачивается:
    // там стол — сам элемент раскладки, и обёртка забрала бы его свойства.
    if shrink_wrap {
        let mut wrap = div().flex().flex_row();
        if root_table {
            wrap = wrap.w_full();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        return wrap.child(outer).into_any_element();
    }
    outer.into_any_element()
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
