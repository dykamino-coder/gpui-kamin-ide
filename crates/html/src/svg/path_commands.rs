//! Построение tiny-skia-геометрии из общего словаря команд CSS path/shape.

mod geometry;
pub(crate) use geometry::Geometry;
#[cfg(test)]
mod tests;
