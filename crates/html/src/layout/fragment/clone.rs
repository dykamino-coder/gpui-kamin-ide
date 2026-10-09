//! Клонирование коробок по фрагментам (`box-decoration-break`).
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::probe::size_monolith;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::multicol::spanner::multicol_container;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Гибкий контейнер или сетка БЕЗ своей коробки (ни рамок, ни отбивок, ни фона,
/// ни заданной высоты, ни позиционирования) с единственным элементом, у которого
/// `box-decoration-break: clone`. По блочной оси элемент такой обёртки стоит
/// там же и той же высоты, что блок-ребёнок: строка flex одна, её высота —
/// высота элемента; сетка без своих дорожек — один ряд `auto`; поперёк элемент
/// растянут (колонка flex, сетка) или имеет свою ширину (ряд flex). Вернуть
/// элемент, поднятый на место обёртки (с её полями), — тогда клонированное
/// украшение (css-break-4 §break-decoration) фрагментирует сам элемент
/// (`box-decoration-break-clone-018/019/028/029`). Иначе `None`.
pub(crate) fn clone_wrapper_item(w: &Element) -> Option<Element> {
    use crate::style::computed::FlexDir;
    let s = &w.style;
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let b = s.borders();
    let row = match s.display {
        Some(Display::Flex) => matches!(s.flex_dir, None | Some(FlexDir::Row)),
        Some(Display::Grid) => false,
        _ => return None,
    };
    if w.inline
        || s.webkit_box == Some(true)
        || s.vertical == Some(true)
        || s.position.is_some()
        || s.float.is_some_and(|f| f != 0)
        || s.background.is_some()
        || s.bg_image.is_some()
        || s.transform.is_some()
        || s.filter.is_some()
        || s.opacity.is_some()
        || s.flex_wrap == Some(true)
        || s.grid_tracks.is_some()
        || s.grid_cols.is_some()
        || s.grid_areas.is_some()
        || s.align_items.is_some()
        || s.justify_content.is_some()
        || !matches!(s.height, None | Some(Len::Auto))
        || s.min_height.is_some()
        || s.max_height.is_some()
        || ![&s.padding.top, &s.padding.bottom, &s.padding.left, &s.padding.right, &b.top, &b.bottom, &b.left, &b.right]
            .into_iter()
            .all(zero)
        || multicol_container(s)
    {
        return None;
    }
    let mut kids = w.children.iter().filter(|n| !is_blank(n));
    let Some(Node::Element(item)) = kids.next() else {
        return None;
    };
    if kids.next().is_some()
        || item.inline
        || out_of_flow(&item.style)
        || clone_dec(item).is_none()
        || item.style.align_self.is_some()
        || item.style.order.is_some()
        || !zero(&item.style.margin.top)
        || !zero(&item.style.margin.bottom)
        || (row && !matches!(item.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))))
    {
        return None;
    }
    let mut item = item.clone();
    item.style.display = Some(Display::Block);
    item.style.margin.top = s.margin.top;
    item.style.margin.bottom = s.margin.bottom;
    Some(item)
}

/// Монолит по css-break-4 §4.1 (Blink `IsMonolithic`): замещаемый,
/// атомарный строчный, прокручиваемый, `break-inside: avoid`,
/// строчное содержимое (строк укладка не видит) — пустая
/// коробка монолитом НЕ является.
pub(crate) fn solid_box(k: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.display == Some(Display::Block))
    };
    // `contain: size` — монолит и у страниц, и в колонках (`size_monolith`).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): то же БЕЗ роста коробки от
    // вытолкнутого монолита (`705fd58`) и без правил параллельного потока
    // в `shape_full` (диапазоны за заданной высотой; `max-height` у
    // обрезающей коробки). Срез css-break + css-multicol 1495 пар:
    // 472 -> 467, потеряно 5 (`single-line-{column,row}-flex-fragmentation-
    // 010/011/051/063`, `overflow-clip-012` 0.00 -> 0.52): монолит в
    // переполняющем ребёнке выталкивал коробку с ЗАДАННОЙ высотой целиком,
    // а лишняя мера обрезающей коробки рожала колонку. С тремя правилами
    // вместе — замер `scout-break-2026-09e.md` §6.
    size_monolith(k)
        || k.style.break_inside_avoid
        || scrolls(k.style.overflow_x)
        || scrolls(k.style.overflow_y)
        || matches!(
            k.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        // Таблица и ячейка монолитами НЕ являются
        // (css-break-4 §4.1: монолитен замещаемый,
        // прокручиваемый и `break-inside: avoid`);
        // строка таблицы — да, но её не режет и укладка.
        || matches!(
            k.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
        || (k.children.iter().any(|n| !is_blank(n))
            && !k.children.iter().any(block_kid))
}

/// `box-decoration-break: clone` (css-break-4 §break-decoration): блочное
/// украшение `(верх, низ)` — рамка с отбивкой, повторяемые в КАЖДОМ
/// фрагменте. Поле не входит: §break-margins «Cloned margins are always
/// truncated to zero». Длины не в точках — ноль, как в `shape_full`
/// (нестрогий `px_or`). `None` — прежний путь `slice` до последней строки.
/// Ворота — всё, что пара «украшение + поднятое тело» выразить не может:
/// строчная и монолит (свой путь), таблица (своя копия), `border-box`
/// (Blink делит заданную высоту на N·украшение — `clone-007`),
/// многоколоночник с детьми (вложенная стопка), графические эффекты (тело —
/// вложенный клон, эффект лёг бы дважды), позиционированная коробка и
/// внепоточный потомок (`clone-005.tentative`: содержащим блоком стало бы
/// поднятое тело без отбивки).
pub(crate) fn clone_dec(c: &Element) -> Option<(f32, f32)> {
    use crate::style::computed::Position;
    if !c.style.bdb_clone {
        return None;
    }
    fn oof_inside(e: &Element) -> bool {
        e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style) || oof_inside(k)))
    }
    if c.inline
        || solid_box(c)
        || table_box(c)
        || c.style.border_box == Some(true)
        || (multicol_container(&c.style) && c.children.iter().any(|n| !is_blank(n)))
        || c.style.transform.is_some()
        || c.style.filter.is_some()
        || c.style.mask_image.is_some()
        || c.style.opacity.is_some_and(|o| o < 1.0)
        || matches!(
            c.style.position,
            Some(Position::Sticky) | Some(Position::Absolute) | Some(Position::Fixed)
        )
        || oof_inside(c)
    {
        return None;
    }
    let px_of = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    let dt = px_of(&c.style.padding.top) + px_of(&b.top);
    let db = px_of(&c.style.padding.bottom) + px_of(&b.bottom);
    // Нулевое украшение тоже клонируется, если фрагменту есть что рисовать
    // своё: «'box-shadow' … applied to each fragment independently», «A
    // no-repeat background image will thus be rendered once in each
    // fragment» (§break-decoration). `clone-009`: у каждого квадрата свои тень
    // и контур; маска `slice` срезала бы их у обоих.
    let paints = !c.style.shadows.is_empty()
        || c.style.outline.is_some()
        || c.style.bg_image.is_some()
        || c.style.gradient.is_some();
    (dt + db > 0.01 || paints).then_some((dt, db))
}

/// Копия ОДНОГО фрагмента коробки с `box-decoration-break: clone`
/// (css-break-4 §break-decoration: «Each box fragment is independently
/// wrapped with the border, padding … The background is drawn independently
/// in each fragment»). Три коробки:
/// * `deco` — исходная коробка блоком высоты ЭТОГО фрагмента `fh`: рамка,
///   отбивка, фон, тень, скругление — свои у фрагмента;
/// * обёртка `overflow-y: clip` высотой `clip` — «сколько содержимого съел
///   этот фрагмент» (у последнего — до конца видимого переполнения:
///   Blink `fragmentation_utils.cc:435` «child content may overflow it»,
///   `clone-002`); по строчной оси не режет (`clone-012` кладёт содержимое
///   второй колонки в первую отрицательным полем);
/// * `body` — исходная коробка без краски и без рамки/отбивки/полей/ширины,
///   поднятая на `from` СДВИГОМ (`position: relative`), а не полем: у обёртки
///   без рамки поле тела схлопнулось бы сквозь неё и увезло обрезку
///   (taffy `block.rs:186`, `Clip` — не скролл-контейнер).
/// Blink устроен так же: фрагмент — свой `PhysicalBoxFragment` со всеми
/// сторонами, краска общим путём (`box_fragment_painter.cc:2322` разводит
/// только `slice`).
pub(crate) fn clone_fragment(c: &Element, dt: f32, db: f32, from: f32, fh: f32, clip: f32) -> Element {
    use crate::style::computed::{Overflow, Position, Sides};
    let mut body = c.clone();
    body.style = c.style.paint_off();
    body.style.padding = Sides::default();
    body.style.border_width = Sides::default();
    body.style.border_visible = [Some(false); 4];
    body.style.margin = Sides::default();
    body.style.width = None;
    body.style.min_width = None;
    body.style.max_width = None;
    body.style.position = Some(Position::Relative);
    body.style.inset = Sides::default();
    body.style.inset.top = Some(Len::Px(-from));
    body.style.bdb_clone = false;
    body.hover = None;
    body.anim = None;
    body.list_item = None;
    let clip_box = Element {
        list_item: None,
        node_id: c.node_id ^ 0x0BDB_C10E_0000_0001,
        anim: None,
        tag: "div".to_string(),
        style: Computed {
            display: Some(Display::Block),
            height: Some(Len::Px(clip.max(0.0))),
            overflow_y: Some(Overflow::Clip),
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: vec![Node::Element(body)],
        attrs: Vec::new(),
        inline: false,
    };
    let mut deco = c.clone();
    deco.style.display = Some(Display::Block);
    deco.style.height = Some(Len::Px((fh - dt - db).max(0.0)));
    deco.style.min_height = None;
    deco.style.max_height = None;
    deco.style.column_count = None;
    deco.style.column_width = None;
    deco.style.column_height = None;
    deco.first_letter = None;
    deco.first_line = None;
    deco.children = vec![Node::Element(clip_box)];
    deco
}
