//! Предикаты флекс-элементов: ограничение внутри, ширина в ряду, класс A, контейнер.

use crate::dom::{Element, Node};
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// В поддереве (до `depth`) — коробка с заданной высотой и содержимым: свой
/// параллельный поток (css-break-3 §3), которого раскрытый элемент ряда не
/// выражает (`flex_row_lines_of`).
pub(super) fn constrained_inside(c: &Element, depth: u8) -> bool {
    depth > 0
        && c.children.iter().any(|n| match n {
            Node::Element(k) => {
                (matches!(k.style.height, Some(Len::Px(_)) | Some(Len::Pct(_)))
                    && k.children.iter().any(|n| !is_blank(n)))
                    || constrained_inside(k, depth - 1)
            }
            _ => false,
        })
}

/// Главный размер элемента многострочного ряда для `flex_row_lines_of`:
/// `flex-basis` (точки/проценты) при `flex-grow: 0`, иначе `width`; проценты —
/// от главного размера контейнера `main`. `None` — размер по содержимому
/// (`auto`/`content`), который гейт не выражает.
pub(super) fn row_item_width(ks: &Computed, main: f32) -> Option<f32> {
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Pct(p)) => Some(p * main),
        _ => None,
    };
    if ks.basis_content == Some(true) {
        return None;
    }
    match ks.flex_basis {
        Some(Len::Auto) | None => px(&ks.width),
        _ => px(&ks.flex_basis),
    }
}

/// Коробка, дающая точку разрыва класса A (css-break-4 §possible-breaks):
/// блочная, в потоке, не плавающая. `Element.inline` ставится по ТЕГУ
/// (`dom.rs` `INLINE_TAGS`), поэтому `<img style="display: block; page: b">`
/// блочным тут признаётся по `display` (`page-name-img-004`: иначе картинка
/// шла анонимным блоком с именем корня и рвала страницу).
pub(crate) fn class_a_box(e: &Element) -> bool {
    let blocky = (!e.inline && !inline_display(e))
        || matches!(
            e.style.display,
            Some(Display::Block)
                | Some(Display::Flex)
                | Some(Display::Grid)
                | Some(Display::Table)
                | Some(Display::ListItem)
        );
    blocky
        && !out_of_flow(&e.style)
        && e.style.float.unwrap_or(0) == 0
        && !matches!(
            e.style.display,
            Some(Display::None) | Some(Display::Contents)
        )
}

/// Флекс- и грид-контейнер: его дети — элементы раскладки, не блоки потока.
/// 'page' применяется только к коробкам с точками разрыва класса A
/// (css-page-3 §page-prop «Applies to: boxes that create class A break
/// points»), и имя элемента флекса/грида контейнеру не передаётся и
/// разрыва между элементами не ставит (`page-name-flex-001/002-print`:
/// эталон без разрывов). Внутри элемента — обычный блочный поток
/// (`page-name-flex-004-print`).
pub(crate) fn item_container(e: &Element) -> bool {
    matches!(
        e.style.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
    )
}

/// Строчный уровень по `display` у элемента с блочным тегом: `<div
/// style="display: inline-block">` стоит в строке и точки класса A не даёт
/// (css-display-3 §inner-outer; `page-name-inline-block-003-print`: два
/// таких `div` с разными `page` — одна строка, без разрыва).
pub(crate) fn inline_display(e: &Element) -> bool {
    // `display: inline` у блочного тега хранится как `InlineBlock` с меткой
    // `inline_display`: блоки внутри такого строчного разрывают его
    // (block-in-inline), и их разрывы — точки класса A
    // (`css-break/block-in-inline-015-print`). Его не трогаем.
    e.style.inline_display != Some(true)
        && matches!(
            e.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        )
}
