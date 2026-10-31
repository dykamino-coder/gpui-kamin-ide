//! Значения времени и выборка CSS-анимаций в вычисленный стиль.
//! Построение элементов и применение кадра к узлу принадлежат потребителям.

pub mod easing;
mod interpolation;
mod sampling;

pub use sampling::sample_at;
