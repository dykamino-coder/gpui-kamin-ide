//! Изменение размера мышью — `resize`.
//!
//! Как и переход, требует памяти между кадрами: размер, заданный пользователем,
//! должен пережить перерисовку. Память элемента — единственное такое место в
//! GPUI, поэтому это свой `Element`, а не стиль.
//!
//! Ручка рисуется в углу самим элементом; тянуть её можно по той оси, которую
//! разрешил CSS.

pub(crate) mod physical_atomic_frame;
pub(crate) mod vertical_line_baseline;
pub(crate) use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Styled, Window, px,
};
pub(crate) use std::rc::Rc;

pub(crate) mod spot_geometry;
pub(crate) mod rectangular_clip;
pub(crate) mod mask_geometry;
pub(crate) mod legacy_clip;
pub(crate) mod mask_size;
pub(crate) mod polygon_clip;
pub(crate) mod orthogonal_measure;
pub(crate) mod vertical_style;
pub(crate) mod combined_geometry;
pub(crate) mod gap_segments;
pub(crate) mod gap_fragment_tail;
pub(crate) mod transform_geometry;
pub(crate) use gap_segments::segments;
pub(crate) use transform_geometry::quarter_turn;
pub(crate) use crate::interactive::resizable::*;
pub(crate) use crate::interactive::sticky::element::*;
pub(crate) use crate::paint::effects::grouped_element::*;
pub(crate) use crate::paint::effects::transformed_element::*;
pub(crate) use crate::paint::effects::underlay::*;
pub(crate) use crate::paint::effects::mask::element::*;
pub(crate) use crate::paint::effects::filter::*;
pub(crate) use crate::interactive::scroll_area::*;
pub(crate) use crate::interactive::frame::*;
pub(crate) use crate::layout::table::paint::*;
pub(crate) use crate::paint::gap_rules::*;
pub(crate) use crate::paint::gap_rules::geometry::*;
pub(crate) use crate::paint::gap_rules::painter::*;
pub(crate) use crate::text::clamp::*;
pub(crate) use crate::text::vertical::*;
pub(crate) use crate::layout::positioned::containing_block::*;

