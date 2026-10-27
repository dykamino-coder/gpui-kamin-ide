//! TRANSITIONAL(F2-7): DOM-dispatch строит preplan до раскладки форматирующих контекстов; удалить в P22 после перевода всех readers на child resume в P21.
//! Мера содержимого коробки для фрагментации (`shape_contents`, вход `shape_full`): по DOM и
//! стилю без раскладки — высота, поля, точки разреза, принудительные разрывы и монолиты.
//! Выбор ветки: коробка из одних флоатов, таблица (у колонок — отказ), многострочный flex-ряд
//! (`flex_lines`), строчный набор по строкам (`line_shape`), иначе стопка детей
//! (`kids`, `kid_shapes`, `lines`, `height`, `cuts`): схлопнутые поля, `row-gap` flex и сетки,
//! ряд без переноса как параллельные потоки, дотяг внепоточных потомков. Коробки с флоатами
//! внутри меры не имеют (`None`).

use crate::document::state::ValueCx;

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::float::{float_only_box, has_float};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::line_shape::{inline_content, line_run_shape};
use crate::layout::fragment::oof_reach::Measure;
use crate::layout::fragment::table_bands::table_box;
use crate::style::computed::{Display, LenCx};
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

/// Форма содержимого коробки `c` для фрагментации: высота, поля, точки
/// разреза, монолиты, дотяг внепоточных.
///
/// Не собирать строки гибкого контейнера с `flex-wrap` при фрагментации
/// (`flex_lines`/`flex_item_main_w`, `ShapeCx::col_w`, `wrap_end`, ветка
/// «колонка = параллельные потоки, ряд = стопка строк») без меры по строкам:
/// строки собираются, но контейнер с переносом теряет высоту фрагмента, и
/// пары, которые держались стопкой детей, разваливаются
/// (`multi-line-column-flex-fragmentation-*`,
/// `multi-line-row-flex-fragmentation-*`). Половинить нельзя.
/// Мера содержимого коробки с её внепоточной досягаемостью (`Measure`).
pub(super) fn shape_contents(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Option<Measure> {
    let sizes = BoxDimensions::for_element(c, &c.style.used());
    let none = Measure::plain;
    // Кадр меры строк (наследование и ширина) — только при `with_lines`.
    let lines = crate::layout::fragment::LineCx::child(c, vcx);
    let vcx = vcx.with_lines(lines.as_ref().or(vcx.lines));
    // Нет значения — ноль; нерешённое: `strict` (высота) — непроходимо,
    // иначе (поля, отступы, рамки) ничего не добавляет.
    let lens = LenCx::box_viewport(cx.viewport, &c.style.used());
    let px_or = |l: &Option<Len>, strict: bool| {
        if strict {
            lens.px_missing_zero(*l, vcx)
        } else {
            Some(lens.px_lossy_zero(*l, vcx))
        }
    };
    // Коробка из одних флоатов (`float_only_box`): мера — высота их ряда,
    // точек разреза внутри нет (флоаты пустые), рамка сверху — монолит, как
    // у общей ветки ниже.
    if matches!(c.style.used().height, None | Some(Len::Auto))
        && sizes.minimum[1].is_none()
        && let Some(tall) = float_only_box(c)
    {
        let b = c.style.used().borders();
        let top = px_or(&c.style.used().padding.top, false)? + px_or(&b.top, false)?;
        let bot = px_or(&c.style.used().padding.bottom, false)? + px_or(&b.bottom, false)?;
        let mt = px_or(&c.style.used().margin.top, false)?;
        let mb = px_or(&c.style.used().margin.bottom, false)?;
        let solid = if top > 0.0 {
            vec![(0.0, top)]
        } else {
            Vec::new()
        };
        return Some(none(crate::layout::fragment::Shape {
            height: top + tall + bot,
            margin_top: mt,
            margin_bottom: mb,
            cuts: Vec::new(),
            forced: Vec::new(),
            solid,
        }));
    }
    if has_float(c, 3) {
        return None;
    }
    // Таблицу не оценивает никто: её подписи, коробку, ряды и ячейки кладёт
    // и записывает только раскладка (`flow::blocks::table`; css-tables-3
    // §fragmentation: ряды сохраняют исходную сетку, подписи — соседи коробки
    // в обёртке). У колонок таблица — отказ меры, её меряет раскладка копии;
    // страницы идут прежней стопкой блоков по тегу.
    if table_box(c) && !cx.paged {
        return None;
    }
    let b = c.style.used().borders();
    let mt = px_or(&c.style.used().margin.top, false)?;
    let mb = px_or(&c.style.used().margin.bottom, false)?;
    // Бюджет разжатия — ОДИН на путь. Мера потока снимает обрезку заданной
    // высотой только у ВЕРХНЕЙ ограниченной коробки: по css-break-3 §3
    // вложенная ограниченная коробка заводит СВОЙ параллельный поток, и её
    // переполнение в поток предка не входит. Модель несёт один `over` на
    // ребёнка стопки — второй поток ей выразить нечем, значит и мерить его
    // нельзя.
    // Без бюджета мера ЭТАЛОНА `flex-item-content-overflow-001-ref` (коробка
    // 70 > элемент 50 > внук 140) росла до 170, эталон разъезжался по двум
    // колонкам (`flex-item-content-overflow-001a/001b/002a/002b`). С бюджетом
    // та же мера даёт ровно 70: `over == h`, ветка потока не включается.
    // Приобретения целы: у них ограниченная коробка на пути ОДНА, а её
    // ребёнок задаёт высоту сам и в неё умещается
    // (`overflowed-block-with-room-after-000`: 70 > 200 > 70+60+70).
    let unclamp = cx.unclamped;
    let cx = ShapeCx {
        unclamped: cx.unclamped && c.style.used().height.is_none(),
        ..cx
    };
    let top = px_or(&c.style.used().padding.top, false)? + px_or(&b.top, false)?;
    let bot = px_or(&c.style.used().padding.bottom, false)? + px_or(&b.bottom, false)?;
    // Строчное содержимое — по строкам, если ширина колонки известна.
    if !cx.paged
        && inline_content(c)
        && matches!(
            c.style.used().display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
        && let Some(s) = line_run_shape(c, top, bot, mt, mb, vcx)
    {
        return Some(none(s));
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
        c.style.used().display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && c.style.used().webkit_box != Some(true)
        && !(matches!(
            c.style.used().flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        ) && c.style.used().flex_wrap != Some(true));
    let flex_col = flex_items
        && !c.style.cascaded().wm().vertical()
        && c.style.used().flex_wrap != Some(true)
        && matches!(
            c.style.used().flex_dir,
            Some(crate::style::computed::FlexDir::Col)
                | Some(crate::style::computed::FlexDir::ColReverse)
        );
    let flex_gap = match c.style.used().gap {
        Some((Some(Len::Px(v)), _)) if flex_items && !c.style.cascaded().wm().vertical() => {
            v.max(0.0)
        }
        _ => 0.0,
    };
    if flex_items {
        kids.sort_by_key(|n| match n {
            Node::Element(e) => e.style.used().order.unwrap_or(0),
            Node::Text(_) => 0,
        });
    }
    let (oof_kid, abs_top, avoid_chains, avoid_kid, page_kid, page_prev, blk_avoid) =
        kid_break_flags(c, cx, &kids, flex_items, vcx);
    // Элемент в ОДНОЙ строке с предыдущим — там, где это видно без раскладки:
    // ширины в процентах без полей, отступов, рамок по главной оси, без
    // `flex-basis`, `min/max-width` и `column-gap` (css-flexbox-1 §9.3: строка
    // набирается, пока следующий элемент помещается). Граница внутри строки —
    // не точка класса A (§12: «Class A break opportunities occur between
    // sibling flex lines»; Blink кладёт `break-*` на СТРОКУ,
    // `flex_layout_algorithm.cc:1892-1906`): сцепка от неё начаться не может
    // (`multi-line-row-flex-fragmentation-040`: 50% + 50%).
    let line_inner = line_inner_kids(c, &kids, avoid_chains, vcx);
    // Спуск — по физике контейнера, а не гейтом «только
    // блочный поток»: flex-колонки, flex с переносом и
    // ВЛОЖЕННЫЙ многоколонник без спуска теряют высоту и
    // вылетают из укладки целиком.
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
        vcx,
    )
}
