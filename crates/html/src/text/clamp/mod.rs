//! Обрезка строк `line-clamp` (css-overflow-4 §5.3): фаза layout/paint поверх построенного
//! дерева. Точка среза меряется пробами кадра (`paragraph_rows`), вычисляется элементом
//! `ClampCut` (`cut_element`), хранится между кадрами в состоянии документа (`ClampState`,
//! доступ — `line_state`) и применяется на следующем кадре потолком высоты и бюджетом строк
//! абзаца; `extent` оставляет видимым переполнение содержимого ДО точки. Отдельный модуль:
//! состояние живёт между кадрами и не принадлежит ни одному элементу дерева.
mod plan;
pub(crate) use plan::{ClampInput, ClampPlan, ClampPlans};
mod measure;
pub(crate) use measure::ClampMeasure;
mod build;
mod cut_element;
pub(crate) use build::ClampBuild;
mod state;
pub(crate) use state::ClampState;
mod deferred;
pub(crate) use deferred::DeferredClamp;
mod extent;
pub use crate::text::clamp::extent::{extend_clip, hidden_after_point, hide_box_after_point};
mod line_state;
mod paragraph_rows;
pub use crate::text::clamp::cut_element::ClampCut;
pub use crate::text::clamp::line_state::BoxEnds;
pub use crate::text::clamp::line_state::ClampEntry;
pub use crate::text::clamp::line_state::ClampLines;
pub use crate::text::clamp::line_state::clamp_context;
pub use crate::text::clamp::line_state::clamp_cut;
pub use crate::text::clamp::line_state::clamp_extent;
pub use crate::text::clamp::line_state::clamp_lines_for;
pub use crate::text::clamp::line_state::clamp_next_seq;
pub use crate::text::clamp::line_state::clamp_para;
pub use crate::text::clamp::line_state::{ClampSpan, close_clamp_span, open_clamp_span};
pub use crate::text::clamp::paragraph_rows::clamp_empty_probe;
pub use crate::text::clamp::paragraph_rows::clamp_probe;
pub(crate) use paragraph_rows::ParagraphRows;
