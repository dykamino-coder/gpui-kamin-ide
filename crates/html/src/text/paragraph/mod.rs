//! Своя строчная раскладка: разбиение абзаца на строки по правилам CSS.
//!
//! Переносчик GPUI знает одно правило — рвать по границам слов — и обойти его
//! снаружи можно лишь подсказками (нулевой пробел, словосоединитель). Этого
//! хватает на простые случаи и НЕ хватает на те, где ширина и точка разрыва
//! связаны:
//!
//! * `white-space: pre-wrap` — пробел в конце строки ВИСИТ за краем: место
//!   занимает, а перенос не вызывает;
//! * `white-space: break-spaces` — тот же пробел место занимает И даёт точку
//!   разрыва после себя;
//! * `overflow-wrap: break-word` — слово рвётся ТОЛЬКО если иначе не влезает;
//! * `line-break: anywhere` — разрыв где угодно, поверх запретов типографики;
//! * двунаправленный текст — знаки набираются в логическом порядке, а на экран
//!   идут в видимом, причём переставлять надо ГОТОВЫЕ прогоны, иначе рвётся
//!   арабская вязь.
//!
//! Поэтому строки считаются здесь: один раз меряется вся строка (`layout_line`
//! даёт положение каждого знака), по мере накопления ширины выбираются точки
//! разрыва, а на отрисовке каждая строка набирается своим `shape_line` и
//! рисуется на своём месте.

#[cfg(test)]
mod break_spaces_tests;
mod line_types;
mod model;
#[cfg(test)]
mod tests;
pub use crate::text::paragraph::line_types::Align;
use crate::text::paragraph::line_types::ELLIPSIS;
pub use crate::text::paragraph::line_types::Indent;
pub(crate) use crate::text::paragraph::line_types::Line;
use crate::text::paragraph::line_types::SOFT_HYPHEN;
pub use crate::text::paragraph::line_types::Wrap;
pub use crate::text::paragraph::model::Paragraph;

pub mod tabs;

mod atom_fit;
mod atom_placement;
mod content_baselines;
mod controlled_shape;
mod decor;
mod emphasis;
mod hyphen_shape;
mod overflow_marker;
mod ruby_justification;
mod ruby_overhang;
mod selection_geometry;
mod text_raster_origin;
mod vertical_content_baselines;
mod vertical_geometry;
mod vertical_inline;

mod element;
pub(crate) mod probes;
pub use crate::text::paragraph::probes::*;
pub(super) mod measure;
pub use crate::text::paragraph::measure::*;
pub(super) mod breaking;
pub use crate::text::paragraph::breaking::*;
pub(crate) mod justify;
pub(crate) use crate::text::paragraph::justify::*;
pub(super) mod runs;
use crate::text::paragraph::runs::*;
pub(super) mod geometry;
pub use crate::text::paragraph::geometry::*;
mod atoms;
mod clamp;
mod fit;
pub(super) mod paint;
use crate::text::paragraph::paint::*;
