//! Используемые размеры отдельного ребёнка flex/grid.

mod sizing;
pub(super) use sizing::item_sizing;

use crate::dom::Node;
use crate::render::*;
use crate::style::computed::{Computed, Display, FlexDir};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn ordered_element(
    mut e: crate::dom::Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Node {
    e.style.float = None;
    e.style.clear = None;
    e.style.vertical_align = None;
    e.style.flex_item = matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    // Элемент КОЛОНКИ: определён ли главный размер
    // контейнера (css-flexbox-1 §9.8 п.1). От этого зависит,
    // определён ли блок у ЕГО детей — доля высоты внутри
    // элемента колонки без высоты решается как `auto`
    // (Blink `flex_layout_algorithm.cc`:
    // `is_initial_block_size_indefinite`).
    if matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && matches!(
        inherited.flex_dir,
        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
    ) {
        // Абсолют с ОБОИМИ вертикальными отступами и авто-высотой
        // тоже определён: высота выходит из уравнения
        // css-position-3 §4.1 (`top + height + bottom` = блок
        // содержащего, а он у абсолюта всегда определён,
        // css-sizing-3 §4.1 «definite»). Без этого колонка
        // `position: absolute; top: 0; bottom: 0` считалась
        // неопределённой, и основа-доля ребёнка снималась
        // (`percentage-heights-002`: синяя полоса по содержимому,
        // красный фон контейнера под ней).
        let edge = |l: Option<Len>| l.is_some_and(|v| v != Len::Auto);
        let abs_both_insets = matches!(
            inherited.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) && matches!(inherited.height, None | Some(Len::Auto))
            && edge(inherited.inset.top)
            && edge(inherited.inset.bottom);
        let definite = matches!(inherited.height, Some(Len::Px(_)))
            || (matches!(inherited.height, Some(Len::Pct(_))) && inherited.cb_height_def)
            || abs_both_insets
            || inherited.stretched
            || inherited.root_box;
        e.style.flex_main_def = Some(definite);
    }
    // Доля высоты элемента РЯДА при неопределённой высоте
    // контейнера ведёт себя как `auto` (CSS 2.1 §10.5;
    // css-flexbox-1 §9.8: определённой поперечную ось делает
    // лишь определённый размер контейнера), но вычисленное
    // значение — не `auto`, поэтому `stretch` к ней не
    // применяется и работает как `flex-start` (§9.4 п.11,
    // css-align-3 §6.1). Прежде доля решалась от высоты
    // строки (`stretch-requires-computed-auto-size`: красная
    // коробка в полвысоты соседа).
    if matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && matches!(
        inherited.flex_dir,
        None | Some(FlexDir::Row) | Some(FlexDir::RowReverse)
    ) && inherited.vertical != Some(true)
        && e.style.vertical != Some(true)
        && matches!(e.style.height, Some(Len::Pct(_)))
        && !matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
    {
        let edge = |l: Option<Len>| l.is_some_and(|v| v != Len::Auto);
        let cross_definite = matches!(inherited.height, Some(Len::Px(_)))
            || (matches!(inherited.height, Some(Len::Pct(_))) && inherited.cb_height_def)
            || (matches!(
                inherited.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) && edge(inherited.inset.top)
                && edge(inherited.inset.bottom))
            || inherited.stretched
            || inherited.root_box
            || inherited.aspect_ratio.is_some();
        if !cross_definite {
            e.style.height = None;
            let stretch = match e.style.align_self {
                Some(a) => a == crate::style::computed::Align::Stretch,
                None => matches!(
                    inherited.align_items,
                    None | Some(crate::style::computed::Align::Stretch)
                ),
            };
            if stretch {
                e.style.align_self = Some(crate::style::computed::Align::Start);
            }
        }
    }
    // ★ Эти три правила жили в ветке ОБЫЧНОГО потока (`else`
    // ниже) с проверками на Flex/Grid-родителя — и были
    // недостижимы по построению (скаут flexbox: пробы
    // `flex2-colbasis-*` показали, что компенсация основы не
    // действует). Их место — здесь, среди детей ряда/сетки.
    item_sizing(e, inherited, opts)
}
