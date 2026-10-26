//! Раскладка: display/flex/позиционирование/размеры (apply_layout).

use crate::document::state::ValueCx;
use crate::paint::apply::*;
use crate::style::computed::{Align, Computed, Display, FlexDir, Justify, Placement, Position};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

mod alignment;
mod calc_size;
mod display_axes;
mod grid_gap;
use crate::layout::block::sizing::{BoxDimensions, BoxEdges, ratio_as_auto_min};
mod sizing;
use alignment::layout_alignment;
use display_axes::{layout_display, layout_flex_axes};
use grid_gap::layout_grid_gap;
use sizing::layout_sizing;

/// У legacy-блока flex-выравнивание неприменимо: прежний сброс свойств
/// заменён чтением контекста, включая зависимые defaults и grid lanes.
pub(super) fn legacy_block(c: &Computed) -> bool {
    c.webkit_box == Some(true)
        && c.webkit_box_vertical == Some(true)
        && (c.line_clamp.is_some() || c.clamp_by_height())
}

pub(super) fn apply_layout(
    mut d: Div,
    c: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Div {
    let sizes = BoxDimensions::from_computed(c).transfer_ratio(c);
    d = layout_display(d, c);
    // При вертикальном письме оси меняются местами: `row` — это ось СТРОКИ,
    // а она идёт сверху вниз; `column` — ось потока, справа налево
    // (`vertical-rl`) или слева направо (`vertical-lr`).
    let dir = match (c.flex_dir, c.wm().vertical()) {
        // Ось СТРОКИ при вертикальном письме идёт сверху вниз, а
        // `direction: rtl` разворачивает её снизу вверх — как и в обычном
        // письме он разворачивает строку справа налево
        // (`flexbox-writing-mode-005`).
        (Some(d), true) => Some(match (d, c.wm().block_rl()) {
            (FlexDir::Row, _) if c.wm().rtl() => FlexDir::ColReverse,
            (FlexDir::RowReverse, _) if c.wm().rtl() => FlexDir::Col,
            (FlexDir::Row, _) => FlexDir::Col,
            (FlexDir::RowReverse, _) => FlexDir::ColReverse,
            (FlexDir::Col, true) => FlexDir::RowReverse,
            (FlexDir::Col, false) => FlexDir::Row,
            (FlexDir::ColReverse, true) => FlexDir::Row,
            (FlexDir::ColReverse, false) => FlexDir::RowReverse,
        }),
        // Умолчание `flex-direction: row` в стиле НЕ записано, а ось менять
        // всё равно надо: в вертикальном письме строка идёт сверху вниз,
        // значит главная ось гибкого ряда — вертикальная даже без
        // явно заданного flex-direction.
        (None, true) if matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex)) => {
            if c.wm().rtl() {
                Some(FlexDir::ColReverse)
            } else {
                Some(FlexDir::Col)
            }
        }
        (d, _) => d,
    };
    // `sideways-lr`: строчная ось идёт СНИЗУ вверх (css-writing-modes-4
    // §block-flow) — вертикальные результаты перевода осей разворачиваются.
    // Горизонтальные (из `column`) не трогаются: ось блока у slr обычная,
    // слева направо.
    let dir = if c.wm().vertical() && c.wm().sideways_lr() {
        dir.map(|d| match d {
            FlexDir::Col => FlexDir::ColReverse,
            FlexDir::ColReverse => FlexDir::Col,
            other => other,
        })
    } else {
        dir
    };
    // Разворот по `direction: rtl` — только для обычного письма: при
    // вертикальном он уже учтён в переводе осей выше, и второй раз
    // переворачивать нельзя (`flexbox-writing-mode-005`: колонки в
    // `vertical-rl` шли слева направо).
    let rtl_row = c.wm().rtl() && !c.wm().vertical();
    d = layout_flex_axes(d, c, dir, rtl_row);
    d = layout_alignment(d, c, dir, rtl_row, vcx);
    d = layout_grid_gap(d, c, &sizes, vcx);

    // Движок раскладки всегда трактует размер как `border-box`, а CSS по
    // умолчанию — как `content-box`: заданная ширина не включает отступы и
    // рамку. Без компенсации блок с рамкой 4px выходил на 8 точек уже, чем в
    // браузере, и всё правее него уезжало (поймано сравнением с Chrome).
    let edges = BoxEdges::from_computed(c);
    let native_content_box = crate::layout::native_box::native_content_box_with(c, &sizes);
    if native_content_box {
        d.style().content_box = Some(true);
    }
    let [pad_x, pad_y] = edges.content_compensation(c.border_box == Some(true), native_content_box);

    d = layout_sizing(d, c, &sizes, pad_x, pad_y, vcx);
    d
}

#[cfg(test)]
mod tests;
