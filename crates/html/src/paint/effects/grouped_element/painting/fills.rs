//! Заливки по пути под и над буфером группы (резкие тени и контур
//! `corner-shape`): квад цвета через растровую маску контура — тот же путь, что у
//! `div` с фоном и `clip-path: path()`, так что край совпадает с
//! эквивалентной обрезкой до единицы канала.

use crate::style::values::value::Color;
use crate::svg::path_scene::{Scene, device_mask};
use gpui::{Bounds, Pixels, Window};

/// Сцена покрытия в точках слоя и цвет заливки.
type Fill = (Scene, Color);

/// Рисует заливки снизу вверх в области `layer_at`.
pub(super) fn paint_fills(fills: &[Fill], layer_at: Bounds<Pixels>, window: &mut Window) {
    if fills.is_empty() {
        return;
    }
    for (scene, colour) in fills {
        let Some(image) = device_mask(scene, window.scale_factor()) else {
            continue;
        };
        let mask = (image, layer_at, 3);
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
