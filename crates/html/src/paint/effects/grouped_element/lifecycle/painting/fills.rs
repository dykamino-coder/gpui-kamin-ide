//! Заливки по пути под и над буфером группы (резкие тени и контур
//! `corner-shape`): квад цвета через маску `pathdef:` — тот же путь, что у
//! `div` с фоном и `clip-path: path()`, так что край совпадает с
//! эквивалентной обрезкой до единицы канала.

use super::masks::resolve_mask;
use crate::paint::effects::grouped_element::Grouped;
use crate::style::values::value::Color;
use gpui::{Bounds, IntoElement, Pixels, Window};

/// Запись маски `pathdef:правило:d` в точках слоя и цвет заливки.
type Fill = (String, Color);

/// Рисует заливки снизу вверх в области `layer_at`.
pub(super) fn paint_fills(fills: &[Fill], layer_at: Bounds<Pixels>, window: &mut Window) {
    if fills.is_empty() {
        return;
    }
    // Маска разрешается как у группы без своих настроек маски.
    let probe = Grouped::new(gpui::div().into_any_element());
    for (src, colour) in fills {
        let Some(mask) = resolve_mask(
            src,
            &probe,
            layer_at,
            &(layer_at, layer_at),
            window,
            0.0,
            0.0,
            0.0,
            0.0,
        ) else {
            continue;
        };
        let quad = gpui::fill(layer_at, colour.to_hsla());
        window.paint_group(
            layer_at,
            gpui::Corners::default(),
            0.0,
            1.0,
            0,
            &[],
            Some(mask),
            None,
            |w| w.paint_quad(quad),
        );
    }
}
