//! Мера строк для фрагментации: ширина содержимого блока в потоке родителя (`line_content_w`,
//! CSS 2.1 §10.3.3) и род ширины детей flex/сетки (`items_kind`) для кадров `LineFrame`,
//! база элемента колонки flex (`basis_sized`), сплошной строчный набор как монолит до меры
//! строк (`inline_content`) и заливка хвоста разорванного фрагмента (`slack_fill`,
//! css-break-3 §box-splitting). Подмодули: прогон строк (`run`), вложенные ряды (`nested`,
//! `nested_kid`), группировка строчных прогонов (`inline_runs`).

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::fragment::grid_bands::grid_stack;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::page::paged::visible_overflow;
use crate::style::computed::{Align, Display};
use crate::style::values::value::Len;
mod continuation;
pub(crate) use continuation::line_state;
mod run;
pub(super) use run::line_run_shape;
mod nested;
mod nested_kid;
mod resolved_lengths;
pub(crate) use nested::nested_box_w;
pub(crate) use nested::nested_rows_box;
pub(crate) use nested_kid::layout_unit;
pub(crate) use resolved_lengths::{LengthFacts, resolved_lengths};
mod inline_runs;
pub(crate) use crate::layout::fragment::flow::transpose_tree;
pub(crate) use inline_runs::group_inline_runs;
pub(crate) use inline_runs::side_margin_wrap;

/// Ширина содержимого блока в потоке родителя шириной `pw` (CSS 2.1 §10.3.3:
/// `margin-left + border + padding + width + … = containing block width`).
/// Только обычный блок потока — у прочих ширину решает своя раскладка.
pub(super) fn line_content_w(c: &Element, pw: f32) -> Option<f32> {
    let s = &c.style.used();
    let sizes = BoxDimensions::for_element(c, s);
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
    // ребёнок. Концевое поле копии учитывает баланс (`balance_line`); без
    // этого обёртка ломала `multicol-nested-002`.
    let b = s.borders();
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    match s.width {
        Some(Len::Px(w)) => Some(if sizes.border_box {
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
    let s = &*c.style.used();
    if k.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.wm().vertical()
        || s.flex_wrap == Some(true)
        || !matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
        || !matches!(s.height, None | Some(Len::Auto))
    {
        return None;
    }
    let mut kk = k.clone();
    // База в точках здесь не ставится: при `min-height: auto` элемент не
    // меньше своего содержимого (§4.5), а этой меры у нас нет.
    if k.style.used().basis_content != Some(true) {
        return None;
    }
    kk.style.used_mut().height = None;
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
    let s = &c.style.used();
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
        if wide && s.used_border_color(i) != bg {
            return None;
        }
    }
    Some(bg.to_hsla())
}

/// Как ширина детей коробки `c` известна мере строк (`LineFrame::items`).
pub(super) fn items_kind(c: &Element) -> u8 {
    use crate::style::computed::FlexDir;
    let s = &*c.style.used();
    match s.display {
        Some(Display::Flex) => {
            let col = matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
            let stretch = matches!(s.align_items, None | Some(Align::Stretch));
            if col && stretch && !s.wm().vertical() {
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
            if !x.inline || x.style.used().display == Some(Display::Block))
    };
    k.children.iter().any(|n| !is_blank(n)) && !k.children.iter().any(block_kid)
}
