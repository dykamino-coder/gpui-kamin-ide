//! Полосы таблицы при фрагментации.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::table::anon::fixup_table_children;
use crate::layout::table::is_cell;
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Табличная коробка — по тегу или по `display`.
pub(crate) fn table_box(c: &Element) -> bool {
    c.tag == "table"
        || matches!(
            c.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        )
}

/// Мера таблицы для укладки по фрагментаинерам (css-break-4
/// §possible-breaks, класс A: «table row group boxes, table row boxes»;
/// css-tables-3 §fragmentation). Ряды — стопка: высота ряда — наибольшая
/// из мер его ячеек (ячейка — обычная блочная мера с рамкой и отбивкой),
/// между рядами и вокруг них — `border-spacing`, снаружи — отступ и рамка
/// таблицы. `break-inside: avoid` ряда или группы — монолитный диапазон
/// (css-break-4 §breaking-rules, Rule 2), `break-before/after` ряда или
/// группы — принудительный разрыв на границе ряда. `thead` встаёт первым,
/// `tfoot` — последним, как в `table()`. Заданная высота — ПОЛ коробки рядов
/// (CSS 2.1 §17.5.3, css-tables-3 §terminology: `height` относится к table
/// grid box, обёртка лишь несёт подписи); растянутая коробка раздаёт остаток
/// рядам и потому идёт сплошным блоком без внутренних точек. `rowspan`,
/// сросшиеся рамки, вертикальное письмо, монолит внутри при заданной высоте
/// и неизмеримая ячейка — `None`: таблица идёт цельным куском измеренной
/// высоты без точек, как прежде.
pub(super) fn table_shape(c: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    table_shape_bands(c, depth, cx, &mut TableBands::default())
}

/// Полосы первой шапки и первого подвала таблицы в координатах её меры:
/// `(верх секции, высота секции)` и `break-inside: avoid*` секции, плюс
/// вертикальный `border-spacing`. Нужны повтору секций во фрагментах
/// (`repeat_bands`).
#[derive(Default, Clone, Copy)]
struct TableBands {
    pub(crate) head: Option<(f32, f32)>,
    pub(crate) foot: Option<(f32, f32)>,
    pub(crate) head_avoid: bool,
    pub(crate) foot_avoid: bool,
    pub(crate) spacing: f32,
    /// Коробка рядов `[верх, низ)` — без подписей обёртки.
    pub(crate) box_top: f32,
    pub(crate) box_end: f32,
}

/// Повтор секций для стопки: полосы `flow::Repeat` и геометрия укладки.
type RepeatSpec = (
    Option<(f32, f32)>,
    Option<(f32, f32)>,
    crate::layout::fragment::types::RepeatGeom,
);

/// Повтор шапки/подвала таблицы-ребёнка стопки колонок (css-tables-3
/// §repeated-headers; Blink `table_layout_algorithm.cc:1082-1150`): секция
/// повторяется, если у неё `break-inside: avoid*` и блочный размер не больше
/// четверти фрагментаинера («block-size of the section is one quarter or less
/// than that of the fragmentainer»). Размер фрагментаинера Blink знает только
/// вне первого прохода балансировки (`HasKnownFragmentainerBlockSize`), поэтому
/// здесь — только `column-fill: auto` с заданной высотой и без рядов. Ответ —
/// полосы для `flow::Repeat`: шапка `(верх секции, секция + зазор под ней)`,
/// подвал `(верх секции − зазор, зазор + секция)`.
pub(crate) fn repeat_bands(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
) -> Option<RepeatSpec> {
    let per = fixed.filter(|_| rows.is_none() && table_box(c))?;
    let mut b = TableBands::default();
    table_shape_bands(c, 4, ShapeCx::COLUMNS, &mut b)?;
    let max = per / 4.0;
    let head = b
        .head
        .filter(|&(_, h)| b.head_avoid && h > 0.01 && h <= max + 0.01)
        .map(|(at, h)| (at, h + b.spacing));
    let foot = b
        .foot
        .filter(|&(_, h)| b.foot_avoid && h > 0.01 && h <= max + 0.01)
        .map(|(at, h)| (at - b.spacing, h + b.spacing));
    if head.is_none() && foot.is_none() {
        return None;
    }
    let geom = crate::layout::fragment::types::RepeatGeom {
        head: head.map_or(0.0, |h| h.1),
        foot: foot.map_or(0.0, |f| f.1),
        head_end: head.map_or(0.0, |(at, h)| at + h),
        foot_at: foot.map_or(f32::MAX, |(at, _)| at),
        foot_end: foot.map_or(f32::MAX, |(at, h)| at + h),
        box_top: b.box_top,
        box_end: b.box_end,
    };
    Some((head, foot, geom))
}

/// `RepeatGeom` для щупов укладки (`grow_pushed`, план `clone`): та же мера,
/// что у `StackChild` в сборке стопки.
pub(crate) fn repeat_leads(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
) -> crate::layout::fragment::types::RepeatGeom {
    repeat_bands(c, fixed, rows).map_or_else(Default::default, |r| r.2)
}

fn table_shape_bands(c: &Element, depth: u8, cx: ShapeCx, bands: &mut TableBands) -> Option<Shape> {
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    if depth == 0 || c.style.vertical == Some(true) || c.style.border_collapse == Some(true) {
        return None;
    }
    // Заданная высота таблицы БОЛЬШЕ не повод отказаться от меры: она просто
    // ПОЛ коробки рядов (CSS 2.1 §17.5.3 — используемая высота есть большая
    // из заданной и суммы рядов; css-tables-3 §terminology кладёт `height` на
    // table grid box, а §style-overrides отдаёт обёртке только `position`,
    // `float`, `margin`-* и края — `height` среди них НЕТ). Пока отказ стоял,
    // многоколоночник с такой таблицей не фрагментировался ВОВСЕ: снимок
    // `specified-block-size-002` — зелёное (10,67)..(134,566), то есть 403 css
    // высоты во всю ширину при эталоне (10,67)..(134,191); снимок
    // `specified-block-size-003` — колонки 2-4 пусты, 11750 точек красного
    // фона многоколоночника.
    // Проценты и прочие единицы, как и прежде, — отказ от меры целиком:
    // разрешать их некому, а недомер увёл бы разрез не туда.
    let spec_of = |l: &Option<Len>| match l {
        None => Some(None),
        Some(Len::Px(v)) => Some(Some(*v)),
        _ => None,
    };
    let spec_h = spec_of(&c.style.height)?;
    let spec_min_h = spec_of(&c.style.min_height)?;
    let b = c.style.borders();
    let mt = px_of(&c.style.margin.top).unwrap_or(0.0);
    let mb = px_of(&c.style.margin.bottom).unwrap_or(0.0);
    let top = px_of(&c.style.padding.top)? + px_of(&b.top)?;
    let bot = px_of(&c.style.padding.bottom)? + px_of(&b.bottom)?;
    // Умолчание тега `<table>` (2px) приходит каскадом; у `display: table`
    // зазора нет. Презентационные `cellspacing`/`cellpadding` — как в
    // `table()`: ниже авторского, выше умолчания браузера
    // (`block-page-break-inside-avoid-11`: мера 198 при рисунке 192 —
    // ложный разрыв внутри `avoid`).
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..cx
    };
    let is_row = |e: &Element| e.tag == "tr" || e.style.display == Some(Display::TableRow);
    let is_group = |e: &Element| {
        matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
            || e.style.display == Some(Display::TableRowGroup)
            || e.style.row_group_kind.is_some()
    };
    // Дети чинятся ТЕМ ЖЕ `fixup_table_children`, которым их чинит рисователь
    // `table()` (render.rs:13551; css-tables-3 §3 fixup): бесхозная ячейка,
    // блок или текст прямо в таблице получают анонимный ряд, не-ячейка внутри
    // ряда — анонимную ячейку, `display: contents` растворяется, а ряды внутри
    // групп чинит рекурсивный заход. Прежде мера шла по СЫРОМУ дереву, и любой
    // ребёнок, которому нужна анонимная коробка, отдавал `None`; у колонок
    // `None` — отказ от укладки целиком, и многоколоночник не фрагментировался
    // вовсе: первая колонка переполнялась, остальные пустовали
    // (`table-border-000` 6.25 — зелёное до y=566 при коробке до y=191,
    // `table-cell-border-001` 2.08, `overflow-scroll-row`).
    // `fixed` объявлен ДО `parts`: `parts` держит ссылки внутрь него.
    let fixed = fixup_table_children(&c.children);
    // Подпись — НЕ ряд и не группа: она живёт в анонимной ОБЁРТКЕ таблицы, а
    // не в коробке рядов (css-tables-3 §terminology: table wrapper box —
    // «A block container box generated around table grid boxes to account for
    // any space occupied by each table-caption it owns»; table grid box —
    // «A block-level box containing the table-internal boxes, EXCLUDING its
    // captions»). `fixup_table_children` её не чинит (ветка `is_cap` →
    // `out.push(child.clone())`, render.rs:16726), и в разборе ниже она
    // уходила в `_ => return None`: таблица с подписью в колонках не
    // фрагментировалась ВОВСЕ — первая колонка переполнялась, остальные
    // пустовали. Снимок `table-border-004`: красное 41,67..134,191 — три
    // колонки из четырёх пусты (11750 точек фона многоколоночника).
    // Сторона — с самой подписи, при пустоте — с таблицы (наследование
    // `caption-side`), ровно как в рисователе `table()`; порядок внутри
    // стороны — разметки (`sections-and-captions-mixed-order`: сверху 1 и 2,
    // снизу 14 и 15…20).
    let is_cap_kid = |e: &Element| e.tag == "caption" || e.style.is_caption == Some(true);
    let mut caps_top: Vec<&Element> = Vec::new();
    let mut caps_bot: Vec<&Element> = Vec::new();
    // Части в порядке отрисовки: первая заголовочная группа — вперёд,
    // первая подвальная — назад, остальное как в разметке (`table()`).
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        if is_cap_kid(e) {
            if e.style.caption_bottom.or(c.style.caption_bottom) == Some(true) {
                caps_bot.push(e);
            } else {
                caps_top.push(e);
            }
            continue;
        }
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    // `break-before`/`break-after: avoid*` РЯДА — не только своё значение.
    // css-break-4 §break-propagation переносит `break-before` ПЕРВОГО поточного
    // ребёнка на контейнер («a 'break-before' value on a first in-flow child box
    // is propagated to its container. Likewise a 'break-after' value on a last
    // in-flow child box»), а для «parallel layout» разрешает более частное
    // правило — ячейки ряда как раз параллельные потоки. Частное правило берём у
    // эталона: Blink СЛИВАЕТ значения ВСЕХ ячеек ряда в значение ряда,
    // `table_row_layout_algorithm.cc:169-177` (`row_break_before =
    // JoinFragmentainerBreakValues(row_break_before, cell_break_before)` и та же
    // строка для `break-after`), и отдаёт результат наружу на `:255-257`.
    // `edge_avoid` уже делает перенос с КРАЙНЕГО ребёнка ячейки — это Blink'овы
    // `InitialBreakBefore`/`FinalBreakAfter`; остаётся объединение по всем ячейкам.
    fn row_avoid(row: &Element, last: bool) -> bool {
        edge_avoid(row, last)
            || row.children.iter().filter(|n| !is_blank(n)).any(
                |n| matches!(n, Node::Element(cell) if is_cell(cell) && edge_avoid(cell, last)),
            )
    }
    // Принудительные `break-before`/`break-after` ячеек — тем же слиянием на
    // ряд (Blink `table_row_layout_algorithm.cc:169-177`,
    // `JoinFragmentainerBreakValues`): `break-before-expansion-001` — ячейка
    // второго ряда с `break-before: column`.
    fn row_force(row: &Element, last: bool) -> bool {
        (if last {
            row.style.break_after_force
        } else {
            row.style.break_before_force
        }) || row
            .children
            .iter()
            .filter(|n| !is_blank(n))
            .any(|n| matches!(n, Node::Element(cell) if is_cell(cell) && edge_break(cell, last)))
    }
    // Плоский список рядов: ряд, № группы, avoid группы, разрывы (свои и
    // группы — на первом/последнем её ряду), запреты разрыва на КРАЯХ ряда
    // (`ab`/`aa` — свои, ячеек и краёв группы).
    struct RowRef<'a> {
        row: &'a Element,
        group: usize,
        avoid: bool,
        fb: bool,
        fa: bool,
        ab: bool,
        aa: bool,
    }
    let mut rows: Vec<RowRef> = Vec::new();
    for (gi, (_, e)) in parts.iter().enumerate() {
        if is_row(e) {
            rows.push(RowRef {
                row: e,
                group: gi,
                avoid: false,
                fb: row_force(e, false),
                fa: row_force(e, true),
                ab: row_avoid(e, false),
                aa: row_avoid(e, true),
            });
            continue;
        }
        let inner: Vec<&Element> = e
            .children
            .iter()
            .filter(|n| !is_blank(n))
            .map(|n| match n {
                Node::Element(r) if is_row(r) => Some(r),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let last = inner.len().saturating_sub(1);
        for (i, r) in inner.iter().enumerate() {
            rows.push(RowRef {
                row: r,
                group: gi,
                avoid: e.style.break_inside_avoid,
                fb: row_force(r, false) || (i == 0 && e.style.break_before_force),
                fa: row_force(r, true) || (i == last && e.style.break_after_force),
                // Край ГРУППЫ — тот же перенос, что у принудительных выше:
                // `break-before: avoid` группы действует на её ПЕРВОМ ряду,
                // `break-after: avoid` — на ПОСЛЕДНЕМ (css-break-4 §break-between,
                // «Applies to: … table row groups, table rows»).
                ab: row_avoid(r, false) || (i == 0 && e.style.break_before_avoid),
                aa: row_avoid(r, true) || (i == last && e.style.break_after_avoid),
            });
        }
    }
    // Таблица ИЗ ОДНИХ ПОДПИСЕЙ мерится: коробка рядов у неё пуста (только
    // рамка и отбивка), но обёртка несёт подписи и их точки разреза.
    // `table-border-004`: подпись 110 + пустая коробка `border-width:20px 0`
    // (20 + 20) + подпись 250 = 400 = ровно четыре колонки по 100.
    // Таблица ИЗ ОДНОЙ ЗАДАННОЙ ВЫСОТЫ мерится так же, как из одних подписей:
    // рядов нет, но коробка есть и её высоту знает стиль
    // (`specified-block-size-002`: пустая `display: table; height: 400px` =
    // ровно 4 колонки по 100; `table-border-007`: рамка 10 + 180 + 10 = 200 =
    // две колонки по 100 — проба `p-tb-007` = 0.00).
    if rows.is_empty()
        && caps_top.is_empty()
        && caps_bot.is_empty()
        && spec_h.is_none()
        && spec_min_h.is_none()
    {
        return None;
    }
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    if top > 0.0 {
        solid.push((0.0, top));
    }
    let mut y = top;
    let mut group_open: Option<(usize, f32)> = None;
    // Сцепка рядов, скованных `break-before/after: avoid*`: начало открытого
    // диапазона, `open` предыдущего ряда и его `break-after: avoid*`.
    let mut avoid_run: Option<f32> = None;
    let mut prev_open = 0.0f32;
    let mut prev_aa = false;
    let mut force_next = false;
    // Ячейки с `rowspan`: (первый ряд, охват, высота содержимого). Их высота
    // НЕ растит свой ряд — она ложится на все охваченные (css-tables-3
    // §height-distribution); мера принимается, только если охват и так
    // вмещает ячейку (сверка после цикла), иначе — прежний отказ. Точки
    // разреза внутри такой ячейки не берутся: где именно внутри охвата
    // стоит её содержимое, мера не знает. Прежде любой `rowspan` отменял
    // меру таблицы целиком, и стол не фрагментировался вовсе
    // (`table-rowspan-001`: пустая ячейка `rowspan=2`, снимок — вторая
    // колонка пуста, стол переполняет первую).
    let mut spans: Vec<(usize, usize, f32)> = Vec::new();
    let mut row_box: Vec<(f32, f32)> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let start = y + spacing;
        let mut h = px_of(&r.row.style.height)?;
        for n in r.row.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(cell) = n else { return None };
            if !is_cell(cell) {
                return None;
            }
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (02.10): урезать охват до оставшихся
            // рядов (охват в один ряд — обычная ячейка) и брать точки и
            // монолиты содержимого охватывающей ячейки в меру (от начала её
            // ряда). Срез css-break/table 136 пар: 64 → 63, ядро 670: 413 →
            // 412 — потеряна `table-cell-expansion-005` (0.00 → «красное
            // видно»), приобретений ноль: монолиты ячейки, которой раскладка
            // отдаёт высоту охвата, закрывали разрез там, где эталон режет.
            let rs = match cell.attr("rowspan").map(str::trim) {
                None => 1,
                Some(v) => match v.parse::<usize>() {
                    Ok(0) => rows.len().saturating_sub(i).max(1),
                    Ok(n) => n.max(1),
                    Err(_) => return None,
                },
            };
            let (ch, _, _, kcuts, kforced, ksolid) = shape_full(cell, depth - 1, cell_cx)?;
            if rs > 1 {
                spans.push((i, rs, ch));
                continue;
            }
            h = h.max(ch);
            // Точки и монолиты ячеек — объединением, как у ряда flex без
            // переноса: рвать нельзя там, где не даёт хоть одна ячейка.
            cuts.extend(kcuts.into_iter().map(|(a, b)| (start + a, start + b)));
            forced.extend(kforced.into_iter().map(|f| start + f));
            solid.extend(ksolid.into_iter().map(|(a, b)| (start + a, start + b)));
        }
        // css-break-4 §unforced-breaks Rule 1: «may break at a class A break point
        // only if all the 'break-after' and 'break-before' values applicable to
        // this break point allow it, which is when at least one of them forces a
        // break or when none of them forbid it». Значит запрет с ЛЮБОЙ из двух
        // сторон границу закрывает, а принудительный разрыв её открывает обратно
        // (§forced-breaks: «a forced break value effectively overrides any avoid
        // break value that also applies at that break point»). До сих пор мера не
        // читала запреты вовсе: точка класса A ставилась между любой парой рядов,
        // и разрез садился между вторым и третьим рядом `break-avoidance-001…006`
        // вместо первого и второго.
        let joined = i > 0 && (prev_aa || r.ab) && !(r.fb || force_next);
        if i > 0 {
            // Класс A между рядами: разрез по концу предыдущего ряда,
            // продолжение с начала этого (зазор остаётся на новой странице —
            // копия разложена целиком, геометрия сходится). На запрещённой
            // границе точки нет.
            if !joined {
                cuts.push((y, start));
            }
            if r.fb || force_next {
                forced.push(y);
            }
        }
        force_next = r.fa;
        // Диапазон `avoid` начинается в точке класса A ПЕРЕД зазором (`y`),
        // а у первого ряда — в нуле: тогда край листа внутри него читается
        // как «разрыв перед таблицей», а не «внутри её верхней рамки» (Blink
        // `FinishFragmentation`: «Avoid breaking inside block-start border …
        // No valid breakpoints there»; `rowgroup-page-break-inside-avoid-1`).
        let open = if i == 0 { 0.0 } else { y };
        // Одного отсутствия точки класса A мало: `fill_at` (flow.rs) режет ПО
        // КРАЮ КОЛОНКИ (`else { Some(at(edge)) }`), а `cuts` читает только на
        // точное совпадение с краем — они дают усечение поля, а не запрет.
        // Разрез останавливает единственная вещь — сплошной диапазон: ветка
        // `k.solid.iter().find(|&&(a, b)| holds(a, b))` уводит срез к НАЧАЛУ
        // диапазона, накрывшего край. Поэтому сцепка скованных рядов идёт ОДНИМ
        // диапазоном, и открывается он на `open` ПРЕДЫДУЩЕГО ряда: разрыв обязан
        // уйти к разрешённой границе ПЕРЕД ним (`break-avoidance-001`: сцепка
        // (50, 150), срез уходит на 50 — css-tables-3 §breaking-rules «insert some
        // vertical gap between the rows located before and at the overflow point»).
        // Вложенность с `break-inside: avoid` ряда и группы законна: `fill_at` на
        // страницах берёт САМЫЙ ВНЕШНИЙ из накрывших край диапазонов.
        if joined {
            if avoid_run.is_none() {
                avoid_run = Some(prev_open);
            }
        } else if let Some(s) = avoid_run.take() {
            solid.push((s, y));
        }
        if r.row.style.break_inside_avoid {
            solid.push((open, start + h));
        }
        if let Some((g, gs)) = group_open
            && g != r.group
        {
            solid.push((gs, y));
            group_open = None;
        }
        if r.avoid && group_open.is_none() {
            group_open = Some((r.group, open));
        }
        prev_open = open;
        prev_aa = r.aa;
        // Полосы первой шапки/подвала (`TableBands`): от верха их первого ряда
        // до низа последнего. Секция-ряд (`is_row` прямо в таблице) — тоже
        // секция своей роли.
        let (kind, sec) = parts[r.group];
        let band = match kind {
            0 => Some((&mut bands.head, &mut bands.head_avoid)),
            2 => Some((&mut bands.foot, &mut bands.foot_avoid)),
            _ => None,
        };
        if let Some((slot, avoid)) = band {
            *slot = Some(match *slot {
                Some((a, _)) => (a, start + h - a),
                None => (start, h),
            });
            *avoid = sec.style.break_inside_avoid;
        }
        row_box.push((start, h));
        y = start + h;
    }
    bands.spacing = spacing;
    // Сверка охватов (см. `spans`): ячейка выше суммы своих рядов с зазорами
    // раздала бы им высоту — этого мера не умеет, отказ как прежде.
    for (i, rs, ch) in spans {
        let last = (i + rs).min(row_box.len()).saturating_sub(1);
        let span_h = row_box[last].0 + row_box[last].1 - row_box[i].0;
        if ch > span_h + 0.01 {
            return None;
        }
    }
    // Сцепка, дожившая до конца коробки рядов, закрывается её низом.
    if let Some(s) = avoid_run {
        solid.push((s, y));
    }
    if let Some((_, gs)) = group_open {
        solid.push((gs, y));
    }
    // Монолит ВНУТРИ коробки рядов (ячейка с `contain: size`, `break-inside:
    // avoid` ряда или группы) вместе с заданной высотой — прежний отказ.
    // Устройства «монолит переполняет колонку» у стопки колонок нет:
    // `flow.rs:826` при `holds(a, b)` и `a <= from` разреза не берёт, а
    // следующая ветка режет по краю колонки прямо сквозь монолит. Проба
    // `target/probe-ftb/p-mo-003.html` — та же геометрия ОДНИМИ блоками
    // (коробка 200 с двумя `contain: size` по 100 в колонках по 60) — даёт
    // «красное видно», то есть даже верная мера рисунка не спасает, а
    // `monolithic-overflow-003` сегодня 2.08: мера сделала бы ЕЙ ХУЖЕ.
    // Единственный диапазон, которого гейт не считает, — верхняя рамка
    // (0, top): её положили ДО цикла рядов.
    // Страницам этот отказ не нужен: у стопки листов переполнение монолитом
    // своё (`ColumnStack::fill_at`, ветка `paged && placed && cur > target`,
    // crbug 1402540), а «монолитом» здесь оказывается уже верхняя рамка любой
    // ячейки (`shape_full` кладёт её сплошным диапазоном — Blink
    // `FinishFragmentation`: «Avoid breaking inside block-start border»).
    // Отказ уводил таблицу с `block-size` в меру стопки блоков по тегу, где
    // высота считается content-box: `table-fragmentation-001b-print` —
    // 336 + отбивка + рамка = 432 вместо 336 по border-box (UA-лист
    // css-tables-3 `table { box-sizing: border-box }`), третий пустой лист.
    let mono_inside = solid.iter().any(|&(a, b)| a > 0.01 || b > top + 0.01);
    if mono_inside && !cx.paged && (spec_h.is_some() || spec_min_h.is_some()) {
        return None;
    }
    // Заданная высота — ПОЛ коробки рядов. `box-sizing` — та же мерка, что в
    // рисователе (`table()`, `table_border_box`): у ТЕГА `<table>` высота по
    // border-box (UA-правило css-tables-3 `table { box-sizing: border-box }`),
    // у `display: table` на прочих тегах — контентная. `min-height` мерится
    // ПОЛНОЙ коробкой ВСЕГДА — так его кладёт `min_fix` в `table()`
    // (css-tables-3 §computing-the-table-height, CSSWG #5336).
    let content_h = y + spacing + bot;
    let edges = top + bot;
    let border_box =
        c.style.border_box == Some(true) || (c.tag == "table" && c.style.border_box.is_none());
    let floor_h = spec_h
        .map(|v| if border_box { v.max(edges) } else { v + edges })
        .into_iter()
        .chain(spec_min_h.map(|v| v.max(edges)))
        .fold(0.0f32, f32::max);
    let h_box = content_h.max(floor_h);
    // Растянутая коробка раздаёт остаток РЯДАМ (CSS 2.1 §17.5.3; css-tables-3
    // §height-distribution-algorithm), и границы рядов уезжают с измеренных
    // мест: точки класса A между ними больше не верны. Такая коробка идёт
    // сплошным блоком — срез по краю колонки есть правило, а не исключение
    // (css-break-4 §4 «slice»). Снимок `specified-block-size-007`: жёлтый ряд
    // (10,67)..(172,316), голубой (10,317)..(172,566) — раздача у нас РОВНАЯ
    // (200/200) при содержимом 1 и 3, и точки на 1 и 4 были бы ложью.
    if h_box > content_h + 0.01 {
        // Ряды растянуты — измеренные полосы секций тоже неверны.
        bands.head = None;
        bands.foot = None;
        cuts.clear();
        forced.clear();
        solid.clear();
        if top > 0.0 {
            solid.push((0.0, top));
        }
    }
    if bot > 0.0 {
        // Нижняя рамка/отбивка таблицы приклеена к монолиту последнего ряда —
        // то же правило, что у блока (`shape_full`, Р4 break-rest): точки
        // разрыва перед block-end рамкой нет (css-break-4 §possible-breaks, класс
        // C — только при ненулевом зазоре; Blink `FinishFragmentation` держит там
        // лишь «last-resort breakpoint»). `table-border-006`: ряды `avoid` 100 и
        // 70, `border-bottom: 30px` в колонке 170 — рамка уходит вместе с
        // последним рядом, а не одна во вторую колонку. Зазор `border-spacing`
        // между рядом и рамкой — та же «без промежутка» граница: рамка таблицы
        // от ряда отделена именно им, а не полем.
        let end_edge = h_box - bot;
        let glue = solid
            .iter()
            .filter(|&&(a, b)| a > 0.01 && (b - (end_edge - spacing)).abs() < 0.01)
            .map(|&(a, _)| a)
            .fold(end_edge, f32::min);
        solid.push((glue, h_box));
    }
    bands.box_top = 0.0;
    bands.box_end = h_box;
    // Подписей нет — коробка рядов и есть вся мера, как прежде.
    if caps_top.is_empty() && caps_bot.is_empty() {
        cuts.retain(|&(need, _)| need > 0.01 && need < h_box - 0.01);
        forced.retain(|&f| f > 0.01 && f < h_box - 0.01);
        return Some((h_box, mt, mb, cuts, forced, solid));
    }
    // Обёртка таблицы — обычная блочная стопка: верхние подписи, коробка
    // рядов, нижние подписи (css-tables-3 §terminology; Blink
    // `table_layout_algorithm.cc:988` «Add all the top captions» и `:1584`
    // «Add all the bottom captions» — обе петли по ВСЕМ подписям своей
    // стороны, секции между ними). Между соседями обёртки — точка класса A
    // (css-break-4 §possible-breaks: «Between sibling boxes of the following
    // types: … in-flow block-level boxes»), поля соседей схлопываются, как в
    // блочной стопке `shape_full`, и подпись поля ИМЕЕТ (Blink `:1195`
    // «Captions allow margins»). Рамка и отбивка самой таблицы подпись не
    // трогают: она вне коробки рядов.
    // `None` в списке — сама коробка рядов; поля у неё НУЛЕВЫЕ, поля таблицы
    // носит обёртка (они уже в `mt`/`mb` и возвращаются наружу).
    let mut cap_slots: Vec<Option<&Element>> =
        Vec::with_capacity(caps_top.len() + caps_bot.len() + 1);
    cap_slots.extend(caps_top.iter().copied().map(Some));
    cap_slots.push(None);
    cap_slots.extend(caps_bot.iter().copied().map(Some));
    let mut wy = 0.0f32;
    let mut wcuts: Vec<(f32, f32)> = Vec::new();
    let mut wforced: Vec<f32> = Vec::new();
    let mut wsolid: Vec<(f32, f32)> = Vec::new();
    let mut prev_mb = 0.0f32;
    let mut through = 0.0f32;
    let mut last_mb = 0.0f32;
    let mut wfirst = true;
    let mut wforce_next = false;
    for slot in cap_slots {
        let (ih, imt, imb, icuts, iforced, isolid, ifb, ifa) = match slot {
            None => (
                h_box,
                0.0,
                0.0,
                std::mem::take(&mut cuts),
                std::mem::take(&mut forced),
                std::mem::take(&mut solid),
                false,
                false,
            ),
            Some(cap) => {
                let (ch, cmt, cmb, ccuts, cforced, csolid) = shape_full(cap, depth - 1, cx)?;
                // Монолитная подпись (`contain: size`, `break-inside: avoid`,
                // прокрутка, замещаемая) — сплошной диапазон во всю высоту,
                // как у любого ребёнка блочной стопки (`shape_full`, ветка
                // `solid_box`).
                let csolid = if solid_box(cap) {
                    vec![(0.0, ch)]
                } else {
                    csolid
                };
                (
                    ch,
                    cmt,
                    cmb,
                    ccuts,
                    cforced,
                    csolid,
                    cap.style.break_before_force,
                    cap.style.break_after_force,
                )
            }
        };
        // У первого поле уходит СКВОЗЬ верх обёртки: своих рамки и отбивки у
        // неё нет (CSS 2.1 §8.3.1).
        let lead = if wfirst {
            through = imt;
            0.0
        } else {
            prev_mb.max(imt)
        };
        if !wfirst {
            wcuts.push((wy, wy + lead));
            if ifb || wforce_next {
                wforced.push(wy);
            }
        }
        wforce_next = ifa;
        let start = wy + lead;
        // Полосы секций — в координаты обёртки: коробка рядов стоит под
        // верхними подписями.
        if slot.is_none() {
            for s in [&mut bands.head, &mut bands.foot].into_iter().flatten() {
                s.0 += start;
            }
            bands.box_top += start;
            bands.box_end += start;
        }
        wcuts.extend(
            icuts
                .into_iter()
                .map(|(need, nf)| (start + need, start + nf)),
        );
        wforced.extend(iforced.into_iter().map(|f| start + f));
        wsolid.extend(isolid.into_iter().map(|(a, b)| (start + a, start + b)));
        wy = start + ih;
        prev_mb = imb;
        last_mb = imb;
        wfirst = false;
    }
    let h = wy;
    wcuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    wforced.retain(|&f| f > 0.01 && f < h - 0.01);
    Some((h, mt.max(through), mb.max(last_mb), wcuts, wforced, wsolid))
}
