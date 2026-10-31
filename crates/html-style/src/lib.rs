//! Внутренняя фаза HTML/CSS: явные контракты конвейера.

#![forbid(unsafe_code)]

pub mod box_facts;
pub mod budget;
pub mod cache;
pub mod color;
pub mod css_syntax;
pub mod defaults;
pub mod diagnostics;
pub mod dom;
pub mod resources;
pub mod style;
pub mod tolerance;
pub mod values;
