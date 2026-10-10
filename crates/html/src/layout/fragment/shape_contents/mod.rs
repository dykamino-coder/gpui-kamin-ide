//! Содержимое формы фрагмента.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::float::{float_only_box, has_float};
use crate::layout::fragment::line_shape::{inline_content, line_run_shape};
use crate::layout::fragment::table_bands::{table_box, table_shape};
use crate::layout::fragment::{LineScope, Shape, ShapeCx};
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod kids;
use kids::{kid_break_flags, shape_from_kids};
mod lines;
use lines::{line_inner_kids, row_axis_flags};
mod cuts;
use cuts::finish_shape_cuts;
pub(crate) use cuts::strip_through_top;
mod kid_shapes;
use kid_shapes::kid_shapes;
mod height;
use height::shaped_height;

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
    let (oof_kid, abs_top, avoid_chains, avoid_kid, page_kid, page_prev, blk_avoid) =
        kid_break_flags(c, cx, &kids, flex_items);
    // Элемент в ОДНОЙ строке с предыдущим — там, где это видно без раскладки:
    // ширины в процентах без полей, отступов, рамок по главной оси, без
    // `flex-basis`, `min/max-width` и `column-gap` (css-flexbox-1 §9.3: строка
    // набирается, пока следующий элемент помещается). Граница внутри строки —
    // не точка класса A (§12: «Class A break opportunities occur between
    // sibling flex lines»; Blink кладёт `break-*` на СТРОКУ,
    // `flex_layout_algorithm.cc:1892-1906`): сцепка от неё начаться не может
    // (`multi-line-row-flex-fragmentation-040`: 50% + 50%).
    let line_inner = line_inner_kids(c, &kids, avoid_chains);
    // Спуск — по физике контейнера. ★ ЗАМЕРЕНО (04.09,
    // срез 1498): без гейта 384, гейт «только блочный
    // поток» 376 (+15/−23) — flex-колонки, flex с
    // переносом и ВЛОЖЕННЫЙ многоколоночник без спуска
    // теряют высоту и вылетают из укладки целиком.
    // Ряд flex без переноса: дети рядом — высота ряда
    // равна наибольшему, точек разреза между ними нет.
    // Сетка и таблица: дети не стопкой, спуска нет.
    let (row_nowrap, grid_rows_stack, row_gap, no_descent) = row_axis_flags(c);
    // (высота, поля, точки, forced, монолиты, force_before,
    //  force_after, ДОТЯГ внепоточного)
    // Дотяг — насколько ниже собственного верха ребёнка
    // уходит низ его внепоточного потомка. В поток он не
    // добавляется (абсолют соседей не двигает), но
    // фрагментация обязана его видеть: css-position-3
    // §abspos-breaking — «The box may subsequently be
    // broken over several fragmentation containers».
    shape_from_kids(
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
        flex_items,
        flex_col,
        flex_gap,
        oof_kid,
        abs_top,
        avoid_kid,
        page_kid,
        page_prev,
        blk_avoid,
        line_inner,
        row_nowrap,
        grid_rows_stack,
        row_gap,
        no_descent,
    )
}
