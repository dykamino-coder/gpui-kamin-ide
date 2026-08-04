//! kamin-model — типизированные DTO протокола/персиста kamin-host (plan/50).
//! Чистый крейт: serde-типы + JSON-инварианты, без транспорта и UI.

pub mod layout;
pub mod sessions;

pub use layout::{ActivitySlot, LayoutSnapshot, merge_shallow};
pub use sessions::{EditorState, OpenFile, Project, SESSION_COLORS, Session, SessionsSnapshot};
