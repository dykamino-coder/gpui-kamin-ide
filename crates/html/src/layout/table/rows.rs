//! Цикл рядов таблицы: ячейки, рамки ячеек, фоны (table_rows).

use crate::render::*;

#[allow(clippy::too_many_arguments, clippy::unnecessary_cast)]
pub(crate) fn table_rows(
    rows: Vec<(&Element, (f32, f32, Option<crate::value::Color>, Option<&Element>))>,
    mut row_ix: i16,
    mut occupied: Vec<u16>,
    opts: &RenderOpts,
    mut under: Vec<AnyElement>,
    group_of: std::collections::HashMap<u64, (&Element, bool, bool)>,
    inherited: &Computed,
    e: &Element,
    cols: u16,
    mut cells: Vec<AnyElement>,
    zero_cols: Vec<bool>,
    rows_left: Vec<usize>,
    cols_collapsed: Vec<bool>,
    collapse_cells: bool,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    table_is_vertical: bool,
    table_font: f32,
    table_family: &String,
    paint_layers: bool,
    cell_bgs: std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    row_elements: Vec<&Element>,
    table_edges: std::rc::Rc<std::cell::RefCell<Vec<crate::interact::EdgeCell>>>,
    col_rects: Vec<Option<crate::interact::RowRects>>,
    col_els: Vec<Option<&Element>>,
    grp_rects: Vec<Option<crate::interact::RowRects>>,
    grp_els: Vec<Option<&Element>>,
    rules_groups: bool,
    mut group_refs: std::collections::HashMap<u64, crate::interact::RefBox>,
    tbl_style: &Computed,
    mut cells_over: Vec<AnyElement>,
    spacing: (f32, f32),
    from_cols: Vec<Option<f32>>,
    col_widths: Vec<(Option<f32>, Option<f32>)>,
    cols_pct: Vec<Option<f32>>,
    bw: [f32; 4],
    outer_win: [f32; 4],
    have_rows: bool,
    px_of: impl Fn(Option<Len>) -> f32,
) -> AnyElement {
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
                collapsed_cell_edge(
                    &mut cell,
                    &cm,
                    &win_edges,
                    &px_of,
                )
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
                d = collapsed_row_edges(
                    row,
                    col_ix,
                    span_cols,
                    cols,
                    &row_style,
                    &table_edges,
                    &px_of,
                    d,
                );
            }
            // Кромки ГРУППЫ РЯДОВ: верх у первого ряда группы, низ у
            // последнего; `rules=groups` даёт тонкую сплошную по умолчанию.
            if collapse_cells && let Some((g, first, last)) = group_of.get(&row.node_id).copied() {
                d = collapsed_group_edges(
                    g,
                    rules_groups,
                    col_ix,
                    span_cols,
                    cols,
                    first,
                    last,
                    inherited,
                    &table_edges,
                    &px_of,
                    d,
                );
            }
            // Кромки КОЛОНКИ (рамка <col>/<colgroup>) — участники разбора
            // сросшихся конфликтов (источник между ячейкой и таблицей):
            // ячейка колонки несёт её кромку на совпадающем со спаном
            // колонки краю; верх/низ — только крайние ряды.
            if collapse_cells {
                d = collapsed_col_edges(
                    &cell_cols,
                    &col_els,
                    row_ix,
                    &row_elements,
                    inherited,
                    &table_edges,
                    &px_of,
                    d,
                );
            }
            // Кромки ГРУППЫ КОЛОНОК — свой источник, слабее колонки и сильнее
            // таблицы (§17.6.2.1 п.4). Без него `<colgroup style="border">` с
            // колонками внутри терял рамку целиком: `col_elements` отдаёт
            // внутренние колонки, а сама группа в разбор не попадала.
            if collapse_cells {
                d = collapsed_colgroup_edges(
                    cell_cols,
                    &grp_els,
                    row_ix,
                    &row_elements,
                    inherited,
                    &table_edges,
                    &px_of,
                    d,
                );
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
    table_finish(
        under,
        paint_layers,
        cell_bgs,
        cells,
        table_edges,
        cells_over,
        e,
        inherited,
        opts,
        row_elements,
        win_edges,
        table_font,
        table_family,
        spacing,
        cols,
        from_cols,
        col_widths,
        cols_collapsed,
        cols_pct,
        bw,
        outer_win,
        have_rows,
        px_of,
    )
}
