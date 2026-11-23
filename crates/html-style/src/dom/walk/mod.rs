//! Обход html5ever-дерева: каскад и сборка узлов (walk).

mod ancestor;
mod descent;
mod engine;
mod enter;
mod full;
mod kids;
mod leave;
mod marker;
mod pending;
mod probe;
mod record;
mod resume;
pub use probe::{Chains, Probed, ProbedRest, probe};
mod sibling_survey;
pub use ancestor::element_attrs;
pub(super) use engine::walk;
pub(super) use full::walk_children;
pub(super) use record::Recorder;

mod text_nodes;

mod cascade;
mod form_appearance;
mod own_vars;

mod animation_frames;
mod details_content;
mod element_sharing;
mod finish_element;
mod pseudo_styles;
mod revalidation;
mod style_normalization;
mod visited_match;

use super::{content, initial_pseudos, language, presentational_hints};
