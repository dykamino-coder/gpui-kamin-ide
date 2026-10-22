//! Геометрия `<basic-shape>` относительно опорной коробки (css-shapes-1 §3, css-shapes-2 §2.4):
//! центр и радиусы `circle()`/`ellipse()`, скруглённый прямоугольник `inset()`/`rect()`/`xywh()`
//! и слов-коробок, контуры `polygon()`/`path()`/`shape()`. Её читают обтекание
//! (`layout::float::shapes`), отрисовка (`paint::background`, `paint::effects`), `svg` и
//! `offset-path` анимаций.

mod circle_ellipse;
mod path;
mod rounded_rect;
mod shape_function;
pub use circle_ellipse::shape_params;
pub(crate) use path::{path_commands, svg_path_of};
pub(crate) use rounded_rect::rrect_of;
pub(crate) use shape_function::shape_commands;
pub use shape_function::shape_to_path;

/// Геометрия флоата в системе его margin-box, всё в CSS-точках.
pub struct ShapeBox {
    pub mw: f32,
    pub mh: f32,
    /// Опорная коробка формы.
    pub rx: f32,
    pub ry: f32,
    pub rw: f32,
    pub rh: f32,
    /// Content-box (для картинки/градиента).
    pub cx: f32,
    pub cy: f32,
    pub cw: f32,
    pub ch: f32,
    /// Радиусы опорной коробки (tl,tr,br,bl), эллиптические.
    pub radius: [(f32, f32); 4],
    pub threshold: f32,
}
