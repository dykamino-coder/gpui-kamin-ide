//! Формы строк и рамки строк.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::grid_bands::grid_stack;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::page::paged::visible_overflow;
use crate::render::is_blank;
use crate::style::computed::{Align, Display};
use crate::style::values::value::Len;
mod run;
pub(super) use run::line_run_shape;
mod nested;
pub(crate) use nested::nested_box_w;
pub(crate) use nested::nested_rows_box;
pub(crate) use nested::nested_rows_shape;
pub(crate) use nested::resolved_lengths;
mod inline_runs;
pub(crate) use inline_runs::group_inline_runs;
pub(crate) use inline_runs::side_margin_wrap;
pub(crate) use inline_runs::transpose_tree;

/// Ширина содержимого блока в потоке родителя шириной `pw` (CSS 2.1 §10.3.3:
/// `margin-left + border + padding + width + … = containing block width`).
/// Только обычный блок потока — у прочих ширину решает своя раскладка.
pub(super) fn line_content_w(c: &Element, pw: f32) -> Option<f32> {
    let s = &c.style;
    // Блочный flex-контейнер и сетка в потоке занимают ширину как блок
    // (css-flexbox-1 §9.2 / css-grid-2 §6.1: «block-level … sized as a
    // block»); ширину ИХ детей решает `items_kind`.
    if c.inline
        || !matches!(
            s.display,
            None | Some(Display::Block)
                | Some(Display::ListItem)
                | Some(Display::Flex)
                | Some(Display::Grid)
        )
        || s.webkit_box == Some(true)
        || s.float.unwrap_or(0) != 0
        || !matches!(
            s.position,
            None | Some(crate::style::computed::Position::Relative)
        )
        || table_box(c)
        || multicol_container(s)
    {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    // Боковые поля копии фрагмента кладёт обёртка (`side_margin_wrap`): корень
    // `layout_as_root` своих полей не читает, а под обёрткой копия — обычный
    // ребёнок. ★ Прежде (03.10) замер обёртки дал `multicol-nested-002` 0.00 ->
    // 2.67 из-за концевого поля в балансе — теперь оно в `balance_line`.
    let b = s.borders();
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    match s.width {
        Some(Len::Px(w)) => Some(if s.border_box == Some(true) {
            (w - edges).max(0.0)
        } else {
            w
        }),
        None | Some(Len::Auto) => {
            Some((pw - px(&s.margin.left)? - px(&s.margin.right)? - edges).max(0.0))
        }
        _ => None,
    }
}

/// Элемент КОЛОНКИ flex без переноса с главным размером по `flex-basis`
/// (css-flexbox-1 §9.2 шаг 3): `flex-basis: content` — по содержимому, и
/// `height` при этом не действует. Контейнер `height: auto` свободного места не
/// даёт, и гибкость базу не меняет (§9.7). `None` — мера по `height`
/// элемента, как прежде.
pub(super) fn basis_sized(c: &Element, k: &Element) -> Option<Element> {
    use crate::style::computed::FlexDir;
    let s = &c.style;
    if k.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.vertical == Some(true)
        || s.flex_wrap == Some(true)
        || !matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
        || !matches!(s.height, None | Some(Len::Auto))
    {
        return None;
    }
    let mut kk = k.clone();
    // База в точках здесь не ставится: при `min-height: auto` элемент не
    // меньше своего содержимого (§4.5), а этой меры у нас нет.
    if k.style.basis_content != Some(true) {
        return None;
    }
    kk.style.height = None;
    Some(kk)
}

/// Хвост непоследнего фрагмента обычной коробки (`flow::StackChild::slack`):
/// фрагмент, разорванный внутри коробки, занимает остаток фрагментаинера
/// (css-break-3 §box-splitting «the box … continues to the end of the
/// fragmentainer»; Blink `fragmentation_utils.cc` «Consumed block-size … is
/// always stretched to the fragmentainers»). Художник хвоста красит его одним
/// цветом по ширине копии — это точно, лишь когда у коробки сплошной фон без
/// картинки и скруглений, а видимые боковые рамки того же цвета. Иначе `None`.
pub(crate) fn slack_fill(c: &Element) -> Option<gpui::Hsla> {
    let s = &c.style;
    let bg = s.background?;
    if s.bg_image.is_some()
        || s.webkit_box == Some(true)
        || [&s.radius.tl, &s.radius.tr, &s.radius.br, &s.radius.bl]
            .into_iter()
            .any(|r| !matches!(r, None | Some(Len::Px(0.0))))
        || !visible_overflow(s)
    {
        return None;
    }
    let b = s.borders();
    for (i, w) in [(1usize, &b.right), (3usize, &b.left)] {
        let wide = match w {
            None => false,
            Some(Len::Px(v)) => *v > 0.0,
            Some(_) => true,
        };
        if wide && s.border_colors[i].or(s.border_color).or(s.color) != Some(bg) {
            return None;
        }
    }
    Some(bg.to_hsla())
}

/// Как ширина детей коробки `c` известна мере строк (`LineFrame::items`).
pub(super) fn items_kind(c: &Element) -> u8 {
    use crate::style::computed::FlexDir;
    let s = &c.style;
    match s.display {
        Some(Display::Flex) => {
            let col = matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
            let stretch = matches!(s.align_items, None | Some(Align::Stretch));
            if col && stretch && s.vertical != Some(true) {
                1
            } else {
                2
            }
        }
        Some(Display::Grid) => {
            if grid_stack(c) && matches!(s.justify_items, None | Some(Align::Stretch)) {
                1
            } else {
                2
            }
        }
        _ => 0,
    }
}

/// Сплошной строчный набор (без блочных детей) — монолит в стопке, ПОКА его
/// строки не измерены (`line_run_shape` дала точки разреза).
pub(crate) fn inline_content(k: &Element) -> bool {
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.display == Some(Display::Block))
    };
    k.children.iter().any(|n| !is_blank(n)) && !k.children.iter().any(block_kid)
}
