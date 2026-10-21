//! Эффекты этапа покраски, требующие отдельного буфера или элемента GPUI: группы
//! (`opacity`, `filter`, `mix-blend-mode`, `isolation`), обрезки (`clip-path`, `clip`,
//! `overflow: clip` с фигурными углами), маски (`mask-image`, `<mask>`/`<clipPath>`),
//! трансформы и фильтры `url(#id)`. На вход — собранный ребёнок и вычисленный стиль, на выход
//! — обёрнутый `AnyElement`, который кладёт буфер в кадр уже с применённым эффектом.

pub(crate) mod containment_paint;
pub(crate) mod corner_clip;
pub mod filter;
mod filter_region;
pub mod grouped;
pub mod grouped_element;
mod legacy_clip;
pub mod mask;
mod mask_geometry;
mod mask_size;
pub(crate) mod paint_scope;
mod polygon_clip;
mod rectangular_clip;
pub mod transform;
mod transform_geometry;
pub mod transformed_element;
pub mod underlay;
