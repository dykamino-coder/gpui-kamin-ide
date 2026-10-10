//! Заливки областей фона и текстовых клипов, отдельно от загрузки изображений.

use super::over;
use super::{Source, key_exif, paint_tiles, source};
use crate::style::computed::{BgRepeat, Computed, Tiling};
use gpui::{Bounds, Pixels, px};

/// Заливка `background-clip: border-area` (css-backgrounds-4): фон «within
/// the area painted by the border» — одноцветный фон тогда просто лежит ПОД
/// краской рамки. Рисуют его те же примитивы, что и рамку (квад, слой рамки,
/// кольца `corner-shape` и `border-shape`), с той же геометрией стиля.
pub(super) fn border_area_fill(c: &Computed) -> Option<crate::style::values::value::Color> {
    if c.bg_clip != Some(crate::style::computed::BgClip::BorderArea) {
        return None;
    }
    flat_fill(c)
}

/// Цвет, которым красится рамка: свой `border-color` поверх заливки
/// `border-area` («ignoring any transparency introduced by border-color»).
pub(crate) fn border_paint(
    c: &Computed,
    colour: crate::style::values::value::Color,
) -> crate::style::values::value::Color {
    match border_area_fill(c) {
        Some(fill) => over(colour, fill),
        None => colour,
    }
}

/// Единственный цвет растра, если все его точки одинаковы и непрозрачны.
pub(super) fn flat_colour(src: &str) -> Option<crate::style::values::value::Color> {
    let Source::Raster(image) = source(src)? else {
        return None;
    };
    let bytes = image.as_bytes(0)?;
    let first = bytes.get(0..4)?;
    if first[3] != 255 || !bytes.chunks_exact(4).all(|p| p == first) {
        return None;
    }
    // Порядок байтов растра — BGRA (см. `border_image::tests`).
    Some(crate::style::values::value::Color {
        r: first[2] as f32 / 255.0,
        g: first[1] as f32 / 255.0,
        b: first[0] as f32 / 255.0,
        a: 1.0,
    })
}

/// Весь фон коробки одним цветом, если он таков: цвет фона, поверх него
/// одноцветный градиент или одноцветный растр, мощённый без зазоров.
/// `None` — фон узорный (или его нет вовсе).
pub(super) fn flat_fill(c: &Computed) -> Option<crate::style::values::value::Color> {
    if c.gradient.is_some() && c.bg_image.is_some() {
        return None;
    }
    let mut fill = c.background.unwrap_or_default();
    if let Some(g) = &c.gradient {
        let one = g.from;
        let flat = !c.gradient_as_tile()
            && g.to == one
            && g.stops.iter().all(|s| s.0 == one)
            && g.stops_px.iter().all(|s| s.0 == one)
            && g.stops_raw.iter().all(|s| s.0 == one);
        if !flat {
            return None;
        }
        fill = over(one, fill);
    }
    if let Some(src) = &c.bg_image {
        let rep = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
        let covers = |t: Tiling| matches!(t, Tiling::Repeat | Tiling::Round);
        if !covers(rep.axis(true)) || !covers(rep.axis(false)) {
            return None;
        }
        fill = over(flat_colour(&key_exif(src, c))?, fill);
    }
    (fill.a > 0.0).then_some(fill)
}

/// Заливка `background-clip: text`, которую несёт цвет глифов.
///
/// Только НЕПРОЗРАЧНАЯ: тогда «цвет поверх заливки» тоже непрозрачен, и
/// повторное слияние стилей (`inline::inherit` зовётся цепочкой) даёт тот же
/// цвет — смешение не копится.
pub(crate) fn text_clip_fill(c: &Computed) -> Option<crate::style::values::value::Color> {
    if c.bg_clip != Some(crate::style::computed::BgClip::Text) {
        return None;
    }
    flat_fill(c).filter(|f| f.a >= 1.0)
}

/// Нарисовать фоновые плитки стиля в ЗАДАННОЙ области.
///
/// Отдельной функцией, а не замыканием слоя: фон РЯДА таблицы рисуется от
/// области ряда, но обрезается прямоугольниками ячеек — вызывающий ставит
/// маску сам и зовёт отрисовку с областью ряда.
pub fn paint_area(c: &Computed, bounds: Bounds<Pixels>, window: &mut gpui::Window) {
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
        paint_tiles(c, view, Some(bounds), window);
        return;
    }
    paint_tiles(c, bounds, None, window);
}
