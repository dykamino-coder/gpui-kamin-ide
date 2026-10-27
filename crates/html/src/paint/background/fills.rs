//! Заливки областей фона и текстовых клипов, отдельно от загрузки изображений.

use super::over;
use super::paint_tiles;
use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::computed::flat_background::flat_fill;
use gpui::{Bounds, Pixels, px};

/// Заливка `background-clip: border-area` (css-backgrounds-4): фон «within
/// the area painted by the border» — одноцветный фон тогда просто лежит ПОД
/// краской рамки. Рисуют его те же примитивы, что и рамку (квад, слой рамки,
/// кольца `corner-shape` и `border-shape`), с той же геометрией стиля.
pub(super) fn border_area_fill(
    c: &Computed,
    vcx: ValueCx<'_>,
) -> Option<crate::style::values::value::Color> {
    if let Some(fill) = c.flat_border_background {
        return Some(fill);
    }
    if c.bg_clip != Some(crate::style::computed::BgClip::BorderArea) {
        return None;
    }
    flat_fill(c, &vcx.background_images())
}

/// Цвет, которым красится рамка: свой `border-color` поверх заливки
/// `border-area` («ignoring any transparency introduced by border-color»).
pub(crate) fn border_paint(
    c: &crate::style::computed::BoxStyle,
    colour: crate::style::values::value::Color,
    vcx: ValueCx<'_>,
) -> crate::style::values::value::Color {
    match c
        .background_paint
        .paints()
        .then(|| border_area_fill(c, vcx))
        .flatten()
    {
        Some(fill) => over(colour, fill),
        None => colour,
    }
}

/// Нарисовать фоновые плитки стиля в ЗАДАННОЙ области.
///
/// Отдельной функцией, а не замыканием слоя: фон РЯДА таблицы рисуется от
/// области ряда, но обрезается прямоугольниками ячеек — вызывающий ставит
/// маску сам и зовёт отрисовку с областью ряда.
pub fn paint_area(
    c: &Computed,
    bounds: Bounds<Pixels>,
    window: &mut gpui::Window,
    vcx: ValueCx<'_>,
) {
    // `background-attachment: fixed` и здесь считается от ОБЛАСТИ ПРОСМОТРА,
    // а красится внутри своей области — та же двухобластная модель, что у
    // обычной коробки (`layer`). Без неё полоса ряда и группы клала плитку от
    // своего верха и уезжала вниз на всю свою высоту
    // (`background-attachment-applies-to-004/005/006`).
    if c.bg_fixed == Some(true) {
        let view = Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: window.viewport_size(),
        };
        paint_tiles(c, view, Some(bounds), window, vcx);
        return;
    }
    paint_tiles(c, bounds, None, window, vcx);
}
