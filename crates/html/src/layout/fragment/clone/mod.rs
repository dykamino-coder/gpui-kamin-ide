//! Клонирование коробок по фрагментам (`box-decoration-break`).

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::probe::size_monolith;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod dec;
pub(crate) use dec::clone_dec;
pub(crate) use dec::clone_fragment;

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
    let s = &*w.style.used();
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
        || s.wm().vertical()
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
        || ![
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
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
        || out_of_flow(&item.style.used())
        || clone_dec(item).is_none()
        || item.style.used().align_self.is_some()
        || item.style.used().order.is_some()
        || !zero(&item.style.used().margin.top)
        || !zero(&item.style.used().margin.bottom)
        || (row
            && !matches!(
                item.style.used().width,
                Some(Len::Px(_)) | Some(Len::Pct(_))
            ))
    {
        return None;
    }
    let mut item = item.clone();
    item.style.used_mut().display = Some(Display::Block);
    item.style.used_mut().margin.top = s.margin.top;
    item.style.used_mut().margin.bottom = s.margin.bottom;
    Some(item)
}

/// Монолит по css-break-4 §4.1 (Blink `IsMonolithic`): замещаемый,
/// атомарный строчный, прокручиваемый, `break-inside: avoid`,
/// строчное содержимое (строк укладка не видит) — пустая
/// коробка монолитом НЕ является.
pub(crate) fn solid_box(k: &Element) -> bool {
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.used().display == Some(Display::Block))
    };
    monolith_box(k)
        || (k.children.iter().any(|n| !is_blank(n)) && !k.children.iter().any(block_kid))
}

/// The css-break-4 §4.1 monoliths among `solid_box`: everything except a
/// box that is solid only because its line boxes are not measured. Such a
/// box keeps its unused block size (a fixed `height` beyond the lines)
/// fragmentable, as Blink fragments the remaining block-size of a block
/// once its content has ended (`FinishFragmentation`).
pub(crate) fn monolith_box(k: &Element) -> bool {
    monolith_blockified(k)
        // Таблица и ячейка монолитами НЕ являются
        // (css-break-4 §4.1: монолитен замещаемый,
        // прокручиваемый и `break-inside: avoid`);
        // строка таблицы — да, но её не режет и укладка.
        || matches!(
            k.style.used().display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
}

/// `monolith_box` коробки, чей строчный уровень блокифицирован (элемент
/// сетки или flex, css-display-3 §2.7: `inline-block` там — блок): атомарной
/// строчной она не является, монолитом её делают только `contain: size`,
/// `break-inside: avoid`, прокрутка и замещаемое содержимое.
pub(crate) fn monolith_blockified(k: &Element) -> bool {
    k.style.used().break_inside_avoid || content_monolith_blockified(k)
}

/// Физически неделимый блокифицированный элемент; avoid остаётся пожеланием.
pub(crate) fn content_monolith_blockified(k: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    // `contain: size` — монолит и у страниц, и в колонках (`size_monolith`).
    // Не вводить его БЕЗ роста коробки от вытолкнутого монолита и без правил
    // параллельного потока в `shape_full` (диапазоны за заданной высотой;
    // `max-height` у обрезающей коробки): иначе монолит в переполняющем
    // ребёнке выталкивает коробку с ЗАДАННОЙ высотой целиком, а лишняя мера
    // обрезающей коробки рожает колонку (`single-line-{column,row}-flex-
    // fragmentation-010/011/051/063`, `overflow-clip-012`).
    size_monolith(k)
        || scrolls(k.style.used().overflow_x)
        || scrolls(k.style.used().overflow_y)
        || matches!(
            k.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
}
