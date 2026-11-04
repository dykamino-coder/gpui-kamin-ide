//! Обход html5ever-дерева: каскад и сборка узлов (walk).

mod ancestor;
mod engine;
mod enter;
mod leave;
mod marker;
mod pending;
mod sibling_survey;
pub use ancestor::element_attrs;
pub(super) use engine::walk_children;

mod text_nodes;

mod cascade;
mod form_appearance;

mod animation_frames;
mod details_content;
mod element_sharing;
mod finish_element;
mod inline_blocks;
mod pseudo_styles;
mod style_normalization;
mod visited_match;

use super::{content, initial_pseudos, language, presentational_hints};
