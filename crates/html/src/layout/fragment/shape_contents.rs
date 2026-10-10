//! Содержимое формы фрагмента.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::float::{block_like_float, float_only_box, has_float};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::flex_lines::{class_a_box, item_container};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::{
    grid_auto_row_bands, grid_row_forced, grid_row_gaps, grid_rows_px, grid_stack,
};
use crate::layout::fragment::line_shape::{basis_sized, inline_content, line_run_shape};
use crate::layout::fragment::probe::forced_opaque;
use crate::layout::fragment::shape_kids::stack_kids;
use crate::layout::fragment::table_bands::{table_box, table_shape};
use crate::layout::fragment::{LineScope, Shape, ShapeCx, fragment_size};
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::page::names::page_names;
use crate::layout::positioned::predicates::OOF_OWN;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v164, `scout-breakcore-2026-09.md` FRAG-FLEX-WRAP,
/// 11 хунков): сбор строк гибкого контейнера с `flex-wrap` при фрагментации
/// (`flex_lines`/`flex_item_main_w`, `ShapeCx::col_w`, `wrap_end`, ветка
/// «колонка = параллельные потоки, ряд = стопка строк», `max(высота, низ
/// содержимого)`). Обещание +2…+9. Полный свод против v36: +4
/// (`multi-line-row-flex-fragmentation-083a…d`) / −15 (`multi-line-column-
/// flex-fragmentation-009/012/014/038`, `multi-line-row-flex-fragmentation-
/// 007/011/018/020/022/023/029` → «красное видно», `-035/-039/-040/-059`).
/// Строки собираются, но контейнер с переносом теряет высоту фрагмента: пары,
/// которые держались стопкой детей, разваливаются. Половинить нельзя (это и
/// есть откат 04.09); брать заново только с мерой по строкам (FRAG-LINES).
pub(super) fn shape_contents(c: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    // Кадр меры строк (наследование и ширина) — только при `with_lines`.
    let _line_frame = LineScope::enter(c);
    let px_or = |l: &Option<Len>, strict: bool| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Vw(k)) if cx.viewport.is_some() => Some(*k * cx.viewport.unwrap().0),
        Some(Len::Vh(k)) if cx.viewport.is_some() => Some(*k * cx.viewport.unwrap().1),
        Some(_) if !strict => Some(0.0),
        _ => None,
    };
    // Коробка из одних флоатов (`float_only_box`): мера — высота их ряда,
    // точек разреза внутри нет (флоаты пустые), рамка сверху — монолит, как
    // у общей ветки ниже.
    if matches!(c.style.height, None | Some(Len::Auto))
        && c.style.min_height.is_none()
        && let Some(tall) = float_only_box(c)
    {
        let b = c.style.borders();
        let top = px_or(&c.style.padding.top, false)? + px_or(&b.top, false)?;
        let bot = px_or(&c.style.padding.bottom, false)? + px_or(&b.bottom, false)?;
        let mt = px_or(&c.style.margin.top, false)?;
        let mb = px_or(&c.style.margin.bottom, false)?;
        let solid = if top > 0.0 {
            vec![(0.0, top)]
        } else {
            Vec::new()
        };
        return Some((top + tall + bot, mt, mb, Vec::new(), Vec::new(), solid));
    }
    if has_float(c, 3) {
        return None;
    }
    // Таблица — своя мера: ряды стопкой, зазоры `border-spacing`, точки
    // класса A между рядами (css-break-4 §possible-breaks). Неизмеримая
    // (сросшиеся рамки, `rowspan`, подпись, заданная высота) идёт прежним
    // путём — стопкой блоков по тегу: `return None` здесь отнимал у КОЛОНОК
    // точки внутри такой таблицы (`border-collapse-001`, флаг выключен:
    // 0.20 → 2.25 между базой v27 и v79).
    // У колонок неизмеримая таблица по-прежнему `None`: сквозной путь по тегу
    // хранит перенос принудительного разрыва ячейки на таблицу
    // (`break-after-table-cell`, `-child`: 0.00 → 2.08 без гейта, срез
    // `L-brk` 2874 пар, +0/−2). Страницы — стопкой блоков.
    if table_box(c) {
        match table_shape(c, depth, cx) {
            Some(s) => return Some(s),
            None if !cx.paged => return None,
            None => {}
        }
    }
    let b = c.style.borders();
    let mt = px_or(&c.style.margin.top, false)?;
    let mb = px_or(&c.style.margin.bottom, false)?;
    // `cellpadding` — только этой коробке (её кладёт `table_shape`).
    let cell_pad = cx.cell_pad;
    // Бюджет разжатия — ОДИН на путь. Мера потока снимает обрезку заданной
    // высотой только у ВЕРХНЕЙ ограниченной коробки: по css-break-3 §3
    // вложенная ограниченная коробка заводит СВОЙ параллельный поток, и её
    // переполнение в поток предка не входит. Модель несёт один `over` на
    // ребёнка стопки — второй поток ей выразить нечем, значит и мерить его
    // нельзя.
    // ЗАМЕРЕНО (10.09, `target/scout-fragparallel-2026-09b.md`): без бюджета
    // мера ЭТАЛОНА `flex-item-content-overflow-001-ref` (коробка 70 >
    // элемент 50 > внук 140) росла 70 -> 170, эталон разъезжался по двум
    // колонкам, и четыре пары `flex-item-content-overflow-001a/001b/002a/
    // 002b` уходили 0.00 -> 0.81. С бюджетом та же мера даёт ровно 70:
    // `over == h`, ветка потока не включается, `StackChild` байт-в-байт
    // прежний.
    // Приобретения целы: у них ограниченная коробка на пути ОДНА, а её
    // ребёнок задаёт высоту сам и в неё умещается
    // (`overflowed-block-with-room-after-000`: 70 > 200 > 70+60+70).
    let unclamp = cx.unclamped;
    let cx = ShapeCx {
        cell_pad: None,
        unclamped: cx.unclamped && c.style.height.is_none(),
        ..cx
    };
    let pad = |l: &Option<Len>| match cell_pad {
        Some(v) if matches!(l, Some(Len::Px(p)) if *p == 1.0) => Some(v),
        _ => px_or(l, false),
    };
    let top = pad(&c.style.padding.top)? + px_or(&b.top, false)?;
    let bot = pad(&c.style.padding.bottom)? + px_or(&b.bottom, false)?;
    // Строчное содержимое — по строкам, если ширина колонки известна.
    if !cx.paged
        && inline_content(c)
        && matches!(
            c.style.display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
        && let Some(s) = line_run_shape(c, top, bot, mt, mb)
    {
        return Some(s);
    }
    let mut kids: Vec<&Node> = c.children.iter().filter(|n| !is_blank(n)).collect();
    // Гибкий контейнер, чьи элементы идут СТОПКОЙ (колонка; перенос по
    // строкам — пока «строка = элемент»). css-flexbox-1 §4.2: «The margins of
    // adjacent flex items do not collapse», и сквозь край контейнера поле
    // элемента не уходит (контейнер — свой контекст); между элементами —
    // `row-gap` (css-align-3 §8.1: главная ось колонки и ось строк переноса —
    // обе блочные). Порядок — визуальный: копия раскладывается уже
    // переставленной (`reorder` в `blocks()`).
    let flex_items = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && c.style.webkit_box != Some(true)
        && !(matches!(
            c.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        ) && c.style.flex_wrap != Some(true));
    let flex_col = flex_items
        && c.style.vertical != Some(true)
        && c.style.flex_wrap != Some(true)
        && matches!(
            c.style.flex_dir,
            Some(crate::style::computed::FlexDir::Col)
                | Some(crate::style::computed::FlexDir::ColReverse)
        );
    let flex_gap = match c.style.gap {
        Some((Some(Len::Px(v)), _)) if flex_items && c.style.vertical != Some(true) => v.max(0.0),
        _ => 0.0,
    };
    if flex_items {
        kids.sort_by_key(|n| match n {
            Node::Element(e) => e.style.order.unwrap_or(0),
            Node::Text(_) => 0,
        });
    }
    let oof_kid: Vec<bool> = kids
        .iter()
        .map(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style)))
        .collect();
    // Абсолют с заданным `top` стоит от верха содержащего блока, а не на
    // статическом месте (CSS 2.1 §10.6.4): его дотяг у страниц отсчитывается
    // от верха коробки. Прежде — от курсора потока, и `top: 0` после блока
    // 250vh тянул лист на 250vh дальше (`fixedpos-008-print`: девять листов
    // вместо шести).
    let abs_top: Vec<bool> = kids
        .iter()
        .map(|n| {
            matches!(n, Node::Element(k)
                if k.style.position == Some(crate::style::computed::Position::Absolute)
                    && matches!(k.style.inset.top, Some(l) if !matches!(l, Len::Auto)))
        })
        .collect();
    // Запреты `break-before/after: avoid*` элементов гибкой стопки — с
    // переносом с крайних потомков (`edge_avoid`). Сцепки из них (`flex_run`
    // ниже) — только у РЯДА С ПЕРЕНОСОМ: там стопка «строка = элемент» идёт по
    // блочной оси в порядке строк. ★ Потери P6 (свод v219): у колонки сцепка
    // уводила `single-line-column-flex-fragmentation-016/017`,
    // `multi-line-column-flex-fragmentation-027/028` — диапазон сцепки для
    // `fill_at` монолит, и в колонке с заданной высотой ветка `overflow_to`
    // держала в одной колонке элемент в 300px, хотя `avoid` запрещает только
    // ТОЧКУ между элементами (css-break-3 §4.4 правило 1; Blink
    // `fragmentation_utils.cc:266-269` лишь снижает её привлекательность).
    // `wrap-reverse` кладёт строки с другого края, а стопка — в порядке DOM
    // (`multi-line-row-flex-fragmentation-050`).
    let avoid_chains = flex_items
        && c.style.vertical != Some(true)
        && matches!(
            c.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap == Some(true)
        && c.style.flex_wrap_reverse != Some(true);
    let avoid_kid: Vec<(bool, bool)> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if avoid_chains => (edge_avoid(k, false), edge_avoid(k, true)),
            _ => (false, false),
        })
        .collect();
    // Те же запреты на границах БЛОЧНЫХ детей (css-break-4 §4.3 правило 1):
    // граница, закрытая `break-after: avoid*` предыдущего или `break-before:
    // avoid*` следующего, точкой разрыва не служит. Разрыв уходит к последней
    // законной точке ВНУТРИ предыдущего ребёнка (Blink `early_break_`,
    // `block_layout_algorithm.cc:1086`; `break-between-avoid-007`: c с
    // `break-before: avoid` после обёрток над a и b — разрыв между a и b).
    // Начальное/конечное имя страницы поточных детей класса A (css-page-3
    // §using-named-pages п. 4): несовпадение конца предыдущего с началом
    // следующего — принудительный разрыв на их границе, и на ЛЮБОЙ глубине
    // (Blink `fragmentation_utils.cc` `CalculateBreakBetweenValue`: имя
    // ребёнка против имени текущего фрагмента контейнера). Только у страниц;
    // `style.page` здесь уже несёт используемое значение (`fill_used_page`).
    let page_kid: Vec<Option<(String, String)>> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if cx.paged && !item_container(c) && class_a_box(k) => {
                Some(page_names(k, ""))
            }
            _ => None,
        })
        .collect();
    let page_prev: Option<String> = None;
    let blk_avoid: Vec<(bool, bool)> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if !flex_items && !out_of_flow(&k.style) => {
                (edge_avoid(k, false), edge_avoid(k, true))
            }
            _ => (false, false),
        })
        .collect();
    // Элемент в ОДНОЙ строке с предыдущим — там, где это видно без раскладки:
    // ширины в процентах без полей, отступов, рамок по главной оси, без
    // `flex-basis`, `min/max-width` и `column-gap` (css-flexbox-1 §9.3: строка
    // набирается, пока следующий элемент помещается). Граница внутри строки —
    // не точка класса A (§12: «Class A break opportunities occur between
    // sibling flex lines»; Blink кладёт `break-*` на СТРОКУ,
    // `flex_layout_algorithm.cc:1892-1906`): сцепка от неё начаться не может
    // (`multi-line-row-flex-fragmentation-040`: 50% + 50%).
    let main_gap0 = match c.style.gap {
        None | Some((_, None)) => true,
        Some((_, Some(Len::Px(v)))) => v.abs() < 0.01,
        _ => false,
    };
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let pct_of = |k: &Element| -> Option<f32> {
        let kb = k.style.borders();
        match k.style.width {
            Some(Len::Pct(p))
                if main_gap0
                    && k.style.flex_basis.is_none()
                    && k.style.min_width.is_none()
                    && k.style.max_width.is_none()
                    && zero(&k.style.margin.left)
                    && zero(&k.style.margin.right)
                    && zero(&k.style.padding.left)
                    && zero(&k.style.padding.right)
                    && zero(&kb.left)
                    && zero(&kb.right) =>
            {
                Some(p)
            }
            _ => None,
        }
    };
    let mut line_acc: Option<f32> = None;
    let mut line_fa = false;
    let line_inner: Vec<bool> = kids
        .iter()
        .map(|n| match n {
            // Внепоточный — не элемент (§4.1): строку не рвёт и в неё не входит.
            Node::Element(k) if avoid_chains && out_of_flow(&k.style) => false,
            Node::Element(k) if avoid_chains => {
                let w = pct_of(k);
                let forced = line_fa || edge_break(k, false);
                line_fa = edge_break(k, true);
                let inner =
                    !forced && matches!((line_acc, w), (Some(s), Some(p)) if s + p <= 1.0 + 1e-3);
                line_acc = match w {
                    Some(p) if inner => line_acc.map(|s| s + p),
                    w => w,
                };
                inner
            }
            _ => {
                line_acc = None;
                line_fa = false;
                false
            }
        })
        .collect();
    // Спуск — по физике контейнера. ★ ЗАМЕРЕНО (04.09,
    // срез 1498): без гейта 384, гейт «только блочный
    // поток» 376 (+15/−23) — flex-колонки, flex с
    // переносом и ВЛОЖЕННЫЙ многоколоночник без спуска
    // теряют высоту и вылетают из укладки целиком.
    // Ряд flex без переноса: дети рядом — высота ряда
    // равна наибольшему, точек разреза между ними нет.
    // Сетка и таблица: дети не стопкой, спуска нет.
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // Сетка в одну колонку с рядами по содержимому — стопка (`grid_stack`):
    // ряд равен одному ребёнку. Без спуска её мера уходила в `grid_rows_px`
    // и почти всегда возвращала `None`, а `None` означает отказ от укладки:
    // многоколоночник с такой сеткой внутри не фрагментировался ВОВСЕ —
    // колонка 1 переполнена, остальные пусты (`scout-break-2026-09f.md` §1;
    // пробы `target/probe-9f/p-grid-item-fragmentation-043.html` и
    // `p-grid-container-fragmentation-009.html` = 0.00 при подмене на блок).
    let grid_rows_stack = grid_stack(c);
    // Зазор рядов такой сетки: `grid_stack` ручается, что он в точках.
    let row_gap = if grid_rows_stack {
        match c.style.gap {
            Some((Some(Len::Px(v)), _)) => v,
            _ => 0.0,
        }
    } else {
        0.0
    };
    // Ряд/группа рядов ВНЕ таблицы (тегом или `display`) — не стопка блоков.
    let no_descent = (matches!(
        c.style.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
            | Some(Display::Table)
            | Some(Display::InlineTable)
            | Some(Display::TableRow)
            | Some(Display::TableRowGroup)
    ) && !grid_rows_stack)
        || matches!(c.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot");
    // (высота, поля, точки, forced, монолиты, force_before,
    //  force_after, ДОТЯГ внепоточного)
    // Дотяг — насколько ниже собственного верха ребёнка
    // уходит низ его внепоточного потомка. В поток он не
    // добавляется (абсолют соседей не двигает), но
    // фрагментация обязана его видеть: css-position-3
    // §abspos-breaking — «The box may subsequently be
    // broken over several fragmentation containers».
    let inner = kid_shapes(c, depth, px_or, cx, &kids, flex_col, no_descent);
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    // Рамка и отбивка самой коробки — без разрывов (Blink:
    // «Avoid breaking inside block-start border»).
    if top > 0.0 {
        solid.push((0.0, top));
    }
    // Стек вложенных: конец, поле первого, схлопнувшееся
    // сквозь верх без отбивки, поле последнего.
    let mut stacked: Option<(f32, f32, f32)> = None;
    // Самый нижний край внепоточных потомков, отсчитанный
    // от верха ЭТОЙ коробки. В поток не входит, высоту
    // соседей не двигает — нужен только фрагментации.
    let mut oof_reach = 0.0f32;
    if let Some(kids) = inner.filter(|k| !k.is_empty()) {
        (stacked, oof_reach) = stack_kids(
            kids,
            row_nowrap,
            top,
            &mut cuts,
            &mut forced,
            &mut solid,
            flex_items,
            oof_kid,
            cx,
            abs_top,
            grid_rows_stack,
            row_gap,
            flex_gap,
            avoid_kid,
            line_inner,
            blk_avoid,
            page_kid,
            oof_reach,
            page_prev,
        );
    }
    // Заданная высота — в точках или (для страниц) в единицах окна.
    let (h, mt, mb) = match shaped_height(
        c,
        depth,
        px_or,
        mt,
        mb,
        unclamp,
        cx,
        top,
        bot,
        kids,
        &mut cuts,
        &mut forced,
        &mut solid,
        stacked,
    ) {
        Ok(value) => value,
        Err(value) => return value,
    };
    // Содержащий блок обязан дотянуться до низа своих
    // внепоточных потомков — только тогда фрагментация
    // родит под них колонки, а балансировка их посчитает
    // (css-position-3 §abspos-breaking; Blink
    // `column_layout_algorithm.cc:1092-1131` прогоняет
    // `OutOfFlowLayoutPart` внутри цикла балансировки
    // именно ради этого). Если коробка содержащим блоком
    // НЕ является, дотяг принадлежит кому-то выше и здесь
    // не учитывается — он всплывёт там.
    let own_h = h;
    let h = if crate::text::inline::establishes_cb(&c.style) {
        h.max(oof_reach + bot)
    } else {
        h
    };
    let h = fragment_size::constrain(h, &c.style, top + bot, cx.viewport, unclamp);
    // Дотяг меняет меру фрагментации, но не размер коробки: сосед в потоке
    // встаёт под КОНЦОМ коробки, а абсолют продолжается параллельным потоком
    // (css-position-3 §abspos-breaking; Blink ведёт OOF во фрагментаинере
    // отдельно от потока). Стопка берёт собственный размер из `OOF_OWN`.
    {
        let own = fragment_size::constrain(own_h, &c.style, top + bot, cx.viewport, unclamp);
        OOF_OWN.with(|m| {
            let mut m = m.borrow_mut();
            if own < h - 0.01 {
                m.insert(c.node_id, (own, h));
            } else {
                m.remove(&c.node_id);
            }
        });
    }
    // css-gaps-1 §fragmentation / css-align-3 §column-row-gap: «the gap
    // disappears when it coincides with a fragmentation break»; Blink
    // `GridLayoutAlgorithm` `MaybeSuppressLastGap`: зазор рядов, в который
    // попал край фрагментаинера (внутрь, на начало или на конец), снимается —
    // следующий ряд начинается с верха следующего фрагмента, линейка в таком
    // зазоре не рисуется (`grid-gap-decorations-fragmentation-001…010`).
    // В стопке колонок это точка класса A с усечением: край внутри
    // `solid`-диапазона зазора уводит разрез к его началу (`fill`, `at(a)`),
    // а `cuts` с тем же `need` продолжает копию с КОНЦА зазора.
    // Keep the cut at the actual gutter start: fill_at's at(edge) handles
    // an exact start boundary. Moving it by a tolerance shortens the painted
    // fragment and can discard a device row. Only extend the end interval
    // for Blink's inclusive last_gap_end_offset >= fragmentainer_space check.
    for (a, b) in grid_row_gaps(&c.style, h - top - bot) {
        cuts.push((top + a, top + b));
        solid.push((top + a, top + b + 0.05));
    }
    // Принудительный разрыв элемента сетки — на границу его РЯДА
    // (css-grid-2 §Fragmenting Grid Layout; Blink `grid_layout_algorithm.cc`
    // `row_break_between`). Сетка без `grid_stack` спуска не знает, и до
    // этого места её `forced` был пуст ВСЕГДА: разрез шёл по краю колонки
    // (`flow.rs: fill_at`, ветка `Some(at(edge))`) —
    // `grid-item-fragmentation-044` резался на 100 при границе ряда 50
    // (снимок `target/wpt-shots/_fg-grid-item-fragmentation-044.png`:
    // красный прямоугольник `x 73..134, y 129..191`). Проба
    // `target/probe-fg/p-fg-044.html` (та же геометрия блоками, разрыв на
    // границе ряда) = 0.00.
    let (row_forced, row_mono) = grid_row_forced(c);
    for f in row_forced {
        forced.push(top + f);
    }
    // Монолитный ряд — целиком (`grid_row_forced`); разрез у его начала
    // растит предыдущую дорожку (`grow_grid_track`), и переполнение
    // элементов предыдущего ряда остаётся в колонке, как у Blink.
    for (a, e) in row_mono {
        solid.push((top + a, top + e));
    }
    // Внутренние принудительные разрывы монолита фрагментации не видны
    // (`forced_opaque`): `fill_at` проверяет `forced` РАНЬШЕ монолитности и
    // разрезал бы `contain: size`-коробку по разрыву её потомка.
    if forced_opaque(c) {
        forced.clear();
    }
    cuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    forced.retain(|&f| f > 0.01 && f < h - 0.01);
    // Монолит ребёнка за ЗАДАННОЙ высотой коробки — параллельный поток, а
    // не запрет разреза в её потоке (css-break-4 §parallel-flows; Blink
    // `FinishFragmentation`, `fragmentation_utils.cc`: «If the block-size is
    // constrained / fixed … we know that we're at the end» — сосед
    // продолжает в той же колонке; `BoxFragmentBuilder::
    // MustStayInCurrentFragmentainer`: «any first piece of child content
    // also needs to stay in the current fragmentainer, even if this causes
    // fragmentainer overflow»). Начатый ниже низа — вычёркивается, начатый
    // выше — бережётся лишь до низа. Без этого `contain: size` в
    // переполняющем ребёнке выталкивал коробку целиком (`single-line-
    // column-flex-fragmentation-051`, `tall-content-inside-constrained-
    // block-*`). У коробки с высотой auto диапазоны и так внутри `h`.
    solid.retain(|&(a, _)| a < h - 0.01);
    for r in solid.iter_mut() {
        r.1 = r.1.min(h);
    }
    bottom_edge_cuts(bot, &mut solid, stacked, h);
    Some((h, mt, mb, cuts, forced, solid))
}

fn kid_shapes(
    c: &Element,
    depth: u8,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    cx: ShapeCx,
    kids: &Vec<&Node>,
    flex_col: bool,
    no_descent: bool,
) -> Option<
    Vec<(
        f32,
        f32,
        f32,
        Vec<(f32, f32)>,
        Vec<f32>,
        Vec<(f32, f32)>,
        bool,
        bool,
        f32,
    )>,
> {
    let inner: Option<Vec<KidShape>> = if depth == 0 || no_descent {
        None
    } else {
        kids.iter()
            .map(|n| match n {
                // Абсолют высоты стопке не даёт и разреза
                // не мешает: нулевая запись, а не отказ
                // от всей укладки (`out-of-flow-in-
                // multicolumn-*`, корень A2). Но НУЛЬ в
                // девятом поле означал бы, что его вовсе
                // нет во фрагментации, а css-position-3
                // §abspos-breaking требует обратного: «an
                // absolutely positioned box is positioned
                // relative to its containing block ignoring
                // any fragmentation breaks (as if the flow
                // were continuous). The box may
                // subsequently be broken over several
                // fragmentation containers». Значит
                // содержащий блок обязан ДОТЯНУТЬСЯ до его
                // низа — иначе колонок под него не
                // родится (Blink
                // `column_layout_algorithm.cc:1125`:
                // `actual_column_count +=
                // column_balancing_info.num_new_columns`).
                // Плавающий сюда не входит: он не
                // позиционированный, и содержащего блока
                // собой не задаёт.
                Node::Element(k) if out_of_flow(&k.style) => {
                    let abs = matches!(
                        k.style.position,
                        Some(crate::style::computed::Position::Absolute)
                    );
                    let reach = if abs {
                        // `top: 100vh` у страниц — от page area (эталоны
                        // `fixedpos-*` ставят копии `top: N00vh`; без этого
                        // досягаемость нулевая, лист один).
                        let top = px_or(&k.style.inset.top, false).unwrap_or(0.0);
                        // Собственная высота абсолюта — той
                        // же мерой: она уже включает дотяг
                        // ЕГО внепоточных потомков, и
                        // цепочка `abs > abs` складывается
                        // сама (`out-of-flow-in-multicolumn-
                        // 022/025`).
                        let own = shape_full(k, depth - 1, cx).map(|s| s.0).unwrap_or(0.0);
                        (top + own).max(0.0)
                    } else {
                        0.0
                    };
                    Some((
                        0.0,
                        0.0,
                        0.0,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        false,
                        false,
                        reach,
                    ))
                }
                Node::Element(k)
                    if !k.inline
                        && (k.style.position.is_none()
                            || k.style.position
                                == Some(crate::style::computed::Position::Relative))
                        && (k.style.float.unwrap_or(0) == 0 || block_like_float(&k.style)) =>
                {
                    // Главный размер элемента КОЛОНКИ flex — его `flex-basis`
                    // (css-flexbox-1 §9.2 шаг 3): `content` — по содержимому,
                    // а `height` при этом не действует; в точках — сама база.
                    // Контейнер `height: auto` свободного места не даёт, и
                    // гибкость базу не меняет (§9.7).
                    let based = if flex_col { basis_sized(c, k) } else { None };
                    let k = based.as_ref().unwrap_or(k);
                    shape_full(k, depth - 1, cx).map(|(h, mt, mb, cuts, forced, solid)| {
                        // Монолит-потомок — весь диапазон
                        // его высоты; иначе — его собственные
                        // монолиты.
                        let solid = if solid_box(k) && !(inline_content(k) && !cuts.is_empty()) {
                            vec![(0.0, h)]
                        } else {
                            solid
                        };
                        (
                            h,
                            mt,
                            mb,
                            cuts,
                            forced,
                            solid,
                            // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                            // передаётся коробке (css-break-4
                            // §break-propagation) — у страниц; колонки
                            // не трогаются (отдельный замер).
                            // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                            // передаётся коробке (css-break-3 §5.1
                            // break-propagation; Blink `InitialBreakBefore`)
                            // одинаково у страниц и у колонок: правило не
                            // про вид фрагментаинера. Гейт `cx.paged` был
                            // «пока не замерено» — `single-line-row-flex-
                            // fragmentation-016` с разрывом на внуке стоит
                            // красной ровно из-за него (проба
                            // `target/probe-9g/p-…-016.html` = 0.00).
                            edge_break(k, false),
                            edge_break(k, true),
                            // Дотяг внепоточных ЭТОГО потомка в
                            // поток родителя не переходит: у
                            // него свой содержащий блок.
                            0.0,
                        )
                    })
                }
                _ => None,
            })
            .collect()
    };
    inner
}

#[allow(clippy::too_many_arguments)]
fn shaped_height(
    c: &Element,
    depth: u8,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    mt: f32,
    mb: f32,
    unclamp: bool,
    cx: ShapeCx,
    top: f32,
    bot: f32,
    kids: Vec<&Node>,
    cuts: &mut Vec<(f32, f32)>,
    forced: &mut Vec<f32>,
    solid: &mut Vec<(f32, f32)>,
    stacked: Option<(f32, f32, f32)>,
) -> Result<(f32, f32, f32), Option<(f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)>> {
    let (h, mt, mb) = match c
        .style
        .height
        .as_ref()
        .map(|_| px_or(&c.style.height, true))
    {
        // Мера потока (`unclamp`) заданной высотой не обрезается:
        // css-break-3 §3 «parallel flows» — переполнение продолжается в
        // следующем фрагментаинере само по себе, и его протяжённость нужна
        // укладке колонок. Высота КОРОБКИ при этом не меняется: её даёт
        // обычная мера (`unclamped: false`), и именно она остаётся шагом для
        // соседа. Ниже по функции тем же `h` усекаются `cuts`/`forced`/
        // `solid` — в мере потока они остаются полными, и это ровно то, что
        // нужно: точки разреза и монолиты хвоста.
        Some(Some(v)) => (
            if unclamp {
                // Отрицательное поле первого ребёнка, схлопнутое сквозь верх
                // коробки (`through`), поднимает всё содержимое: его низ —
                // `end + through` от верха коробки (`css-break/float-001`:
                // коробка `height: 0` с ребёнком 40px и `margin-top: -40px`
                // — содержимое кончается на её верху, а мера давала поток
                // 40, и коробка с края колонки уезжала в следующую).
                fragment_size::border_size(v, &c.style, top + bot)
                    .max(stacked.map_or(0.0, |s| s.0 + s.1.min(0.0)) + bot)
            } else {
                fragment_size::border_size(v, &c.style, top + bot)
            },
            mt,
            mb,
        ),
        Some(None) => return Err(None),
        None => match stacked {
            Some((end, through, last_mb)) => (
                // Нижнее поле последнего ребёнка при собственном нижнем
                // отступе/рамке коробки НЕ схлопывается наружу и входит в
                // высоту (CSS 2.1 §10.6.3, §8.3.1): `table-fragmentation-
                // 001c-ref` — `.table { padding }` + `.td { margin: .25in 0 }`,
                // мера 360 при рисунке 384, нижняя рамка не влезала в копию.
                // У страниц; колонки не трогаются (отдельный замер).
                end + if bot > 0.0 && cx.paged { last_mb } else { 0.0 } + bot,
                mt.max(through),
                if bot == 0.0 { mb.max(last_mb) } else { mb },
            ),
            // Сетка без заданной высоты: её высоту знают
            // ЯВНЫЕ дорожки рядов (`grid-template-rows:
            // 200px`) с зазорами между ними. Без этой
            // оценки укладка колонок отказывалась от всей
            // коробки, и многоколоночник с сеткой внутри
            // уходил в запасную сетку целиком
            // (`scout-break-2026-09b.md`, корень C1).
            // Дорожки идут ПЕРЕД пустотой: у сетки БЕЗ детей высота всё
            // равно есть — её задают дорожки (css-grid-1 §7.1: дорожка
            // существует независимо от того, занята ли она). Прежде
            // `kids.is_empty()` заслонял эту ветку, и
            // `grid-container-fragmentation-002` (`grid-template-rows:
            // 200px`, детей нет) мерился нулём: квадрат 100×100 красен
            // ЦЕЛИКОМ (снимок `target/wpt-shots/_fg-grid-container-
            // fragmentation-002.png`, `x 10..134, y 67..191`). Проба
            // `target/probe-fg/p-fg-002.html` (та же сетка блоком 200px)
            // = 0.00.
            None => match grid_rows_px(&c.style) {
                Some(v) => (v + top + bot, mt, mb),
                // Сетка, у которой дорожки рядов НЕ все в точках (несколько
                // колонок, ряд `auto`, неявный ряд), до сих пор отдавала
                // `return None`. `None` тут стоит дорого: в `blocks()`
                // (:13391) `stackable` собирается через
                // `collect::<Option<Vec<_>>>()`, и одна неизмеримая сетка
                // отменяет укладку колонок ВСЕГО многоколоночника вместе с
                // её соседями — колонка 1 переполнена, остальные пусты
                // (`grid-item-fragmentation-003`: сетка `auto auto` с
                // элементом 200px, ряд 200, ничего не фрагментируется).
                // Высота сетки с `height: auto` — сумма размеров дорожек
                // рядов (css-grid-2 §Fragmenting Grid Layout, шаг 4
                // «Sample Fragmentation Algorithm»; Blink
                // `grid_layout_algorithm.cc:370`
                // `Rows().CalculateSetSpanSize()`). Точек разреза эта ветка
                // не добавляет — см. `grid_auto_row_bands`.
                None => match grid_auto_row_bands(c, depth, cx) {
                    Some((b, spots)) => {
                        // Элементы ряда — параллельные потоки (css-break-3 §3;
                        // Blink `PlaceGridItems`: у каждого свой break token):
                        // точки и монолиты — ОБЪЕДИНЕНИЕМ, со смещением ряда,
                        // как у ряда flex без переноса выше. Рост элемента,
                        // чья строка ушла в следующую колонку, ставит
                        // `grow_pushed` через спуск `pushed_box_at`; после него
                        // строка стоит на краю, и общий срез совпадает с
                        // раздельными (css-grid-2 §12.1 шаг 3: ряд растёт).
                        // Границ РЯДОВ по-прежнему нет — см. выше.
                        for (ix, row, kmt, s, plain) in &spots {
                            // Страницы: монолитный элемент (`contain: size`,
                            // замещаемый…) — сплошной диапазон во всю его
                            // высоту, и край листа внутри него уводит разрыв
                            // к началу ряда (css-grid-2 §fragmenting: «a grid
                            // container may break between rows»; Blink
                            // `IsMonolithic` → разрыв перед рядом;
                            // `grid-fragmentation-between-rows-001-print`:
                            // второй ряд `contain: size` резался краем листа).
                            if cx.paged
                                && !*plain
                                && let (Some(&(r0, _)), Some(Node::Element(k))) =
                                    (b.get(*row), c.children.get(*ix))
                                && solid_box(k)
                            {
                                let start = top + r0 + kmt;
                                solid.push((start, start + s.0));
                                continue;
                            }
                            let (true, Some(&(r0, _))) = (*plain, b.get(*row)) else {
                                continue;
                            };
                            let start = top + r0 + kmt;
                            for (need, nf) in &s.3 {
                                cuts.push((start + need, start + nf));
                            }
                            for f in &s.4 {
                                forced.push(start + f);
                            }
                            for (a, e) in &s.5 {
                                solid.push((start + a, start + e));
                            }
                        }
                        (b.last().map_or(0.0, |r| r.1) + top + bot, mt, mb)
                    }
                    None if kids.is_empty() => (top + bot, mt, mb),
                    None => return Err(None),
                },
            },
        },
    };
    Ok((h, mt, mb))
}

fn bottom_edge_cuts(
    bot: f32,
    solid: &mut Vec<(f32, f32)>,
    stacked: Option<(f32, f32, f32)>,
    h: f32,
) {
    if bot > 0.0 {
        // Между последним поточным ребёнком и нижней отбивкой/рамкой БЕЗ зазора
        // точки разрыва нет: класс C css-break-4 §possible-breaks существует лишь
        // «if there is a (non-zero) gap between them», а Blink держит там только
        // «a last-resort breakpoint before trailing border and padding»
        // (`fragmentation_utils.cc`, `FinishFragmentation`). Край колонки, упавший
        // на начало или внутрь нижней отбивки, обязан увести разрыв к НАЧАЛУ
        // монолита, который к ней примыкает (строка, `inline-block`,
        // `break-inside: avoid`), а не оставить отбивку одну в следующей колонке;
        // рост до низа колонки делает `grow_pushed` (срез на начале монолита).
        // `break-at-end-container-edge-000/001/004`: строки `inline-block` и
        // `padding-bottom` 50/70/60 — последняя строка уходит вместе с отбивкой;
        // `fieldset-005`: две коробки `break-inside: avoid` по 60 и `border-bottom:
        // 40px`. Диапазон из нуля — верхняя рамка самой коробки, его не клеим
        // (пустая коробка с отбивками стала бы монолитом). Нижнее поле последнего
        // ребёнка у колонок в `h` не входит (выше, `cx.paged`), при нём зазор есть
        // — тогда не клеим.
        let end_edge = h - bot;
        let gapless = stacked.is_none_or(|s| s.2.abs() < 0.01);
        let glue = if gapless {
            solid
                .iter()
                .filter(|&&(a, b)| a > 0.01 && (b - end_edge).abs() < 0.01)
                .map(|&(a, _)| a)
                .fold(end_edge, f32::min)
        } else {
            end_edge
        };
        solid.push((glue, h));
    }
}

/// Поле, которое мера схлопнула СКВОЗЬ верх коробки (`shape_full`: у первого
/// поточного блока без верхней отбивки и рамки `through = kmt`, и при высоте
/// `auto` оно уходит в `mt` коробки). Стопка кладёт это поле сама — `lead` в
/// `fill_at`, с усечением на разрыве (css-break-4 `margin-break: auto`: «any
/// margins adjoining the break … are truncated to zero after the break»). Копия
/// же ставится КОРНЕМ
/// (`layout_as_root`), а у корня taffy поля с детьми не схлопывает
/// (`vendor/taffy/src/compute/block.rs:186-192`, `vertical_margins_are_collapsible`
/// у корня ложно): поле внука вставало ВТОРОЙ раз внутри копии. На разрыве это
/// сдвигало содержимое на всё поле вниз: `margin-at-break-001/002` — `margin-top:
/// 60px` у первого внука, в колонке 2 зелёное 110..160 вместо 50..100 (снимок
/// `target/wpt-shots/margin-at-break-001.png`). Снимается ровно цепочка меры:
/// первый непустой ребёнок — поточный блок, у коробки нет верхней отбивки/рамки и
/// высота `auto`. Сетка, гибкий контейнер, таблица, вложенный многоколоночник
/// идут в мере иначе (`grid_rows_stack`, `row_nowrap`, `table_shape`) — не
/// трогаем. Внепоточный первым — у меры нулевая запись, `through` не рождается.
pub(crate) fn strip_through_top(c: &mut Element, depth: u8) {
    if depth == 0
        || c.style.height.is_some()
        || table_box(c)
        || multicol_container(&c.style)
        || c.style.webkit_box == Some(true)
        || !matches!(
            c.style.display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
    {
        return;
    }
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    if px(&c.style.padding.top) + px(&b.top) != 0.0 {
        return;
    }
    let Some(Node::Element(k)) = c.children.iter_mut().find(|n| !is_blank(n)) else {
        return;
    };
    if k.inline
        || out_of_flow(&k.style)
        || !matches!(
            k.style.position,
            None | Some(crate::style::computed::Position::Relative)
        )
    {
        return;
    }
    k.style.margin.top = None;
    strip_through_top(k, depth - 1);
}

type KidShape = (
    f32,
    f32,
    f32,
    Vec<(f32, f32)>,
    Vec<f32>,
    Vec<(f32, f32)>,
    bool,
    bool,
    f32,
);
