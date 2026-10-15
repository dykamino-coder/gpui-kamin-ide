//! Глиф-спиннер «работаю/загружаю» (INC-2026-0001).
//!
//! Вращение не годится для редких кадров: дуга, повёрнутая на 45° за шаг,
//! читается как подтормаживание, а плавное вращение перерисовывает всё окно на
//! каждой развёртке. Смена глифа — дискретна по замыслу (так же спиннер Claude
//! Code: `·` `✢` `✳` `✶` `✻` `✽` и обратно), поэтому ~8 кадров в секунду
//! выглядят задуманно. Шаг берётся с общего такта `anim_tick`: все спиннеры
//! меняются в одном кадре окна.

use gpui::prelude::*;
use gpui::{Div, Rgba, div, px};

/// Кадры: от точки к звезде и обратно.
const FRAMES: [&str; 10] = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
/// Длительность кадра (~8 кадров в секунду).
const STEP_MS: u64 = 120;
/// Кадр при reduced motion (RDP): статичная звезда без потока кадров.
const STILL: &str = "✻";

/// Квадрат `size_px` с текущим глифом цвета `color`.
pub(crate) fn glyph_spinner(size_px: f32, color: Rgba) -> Div {
    let glyph = if crate::win_integration::reduce_motion() {
        STILL
    } else {
        FRAMES[(crate::ui::anim_tick::step(STEP_MS) % FRAMES.len() as u64) as usize]
    };
    div()
        .w(px(size_px))
        .h(px(size_px))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size_px))
        .line_height(px(size_px))
        .text_color(color)
        .child(glyph)
}
