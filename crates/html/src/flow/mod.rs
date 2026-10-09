//! Строчный поток атомов с вырезами (`shape-outside`).
//!
//! Обтекание плавающего блока ФОРМОЙ не выразить рядом-колонкой (наша
//! механика флоатов) и не выразить flex-переносом: у формы ширина выреза
//! своя НА КАЖДОЙ СТРОКЕ. Этот элемент раскладывает готовые коробки
//! построчно сам: курсор, перенос, вырезы по полосам — и рисует детей со
//! смещениями. Дети приходят с ИЗВЕСТНЫМИ размерами (инлайн-блоки с
//! заданными сторонами — ровно то, чем WPT рисует картину обтекания).

pub(crate) mod shapes;
pub(crate) mod rounded_box;
pub use shapes::FloatShape;
pub(crate) use shapes::ellipse_cut;
pub use rounded_box::RoundedBox;

pub(crate) mod fragment_mask;
pub(crate) mod gap_fragment;
pub(crate) mod column_measure;
pub(crate) mod column_baselines;
pub(crate) mod intrinsic_measure;
pub(crate) mod row_element;
pub(crate) mod margin_boxes;
pub(crate) mod margin_box_size;
pub(crate) use margin_boxes::layout_margin_boxes;

pub(crate) use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Size, Window, point, px, size,
};
pub(crate) use crate::layout::fragment::types::*;
pub(crate) use crate::layout::page::page_stack::*;
pub use crate::layout::page::page_stack::{PageGeom, PageGeomFn};
pub(crate) use crate::layout::multicol::column_stack::*;


