//! Значения времени и выборка CSS-анимаций в вычисленный стиль.
//! Построение элементов и применение кадра к узлу принадлежат потребителям.

pub(crate) mod easing;
mod interpolation;
mod sampling;

pub(crate) use sampling::sample_at;
