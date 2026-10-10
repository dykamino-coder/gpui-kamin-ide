//! Обрезка строк `line-clamp`.
mod cut_element;
mod line_state;
mod paragraph_rows;
pub use crate::text::clamp::cut_element::ClampCut;
pub use crate::text::clamp::line_state::ClampEntry;
pub use crate::text::clamp::line_state::ClampGuard;
pub use crate::text::clamp::line_state::ClampLines;
pub use crate::text::clamp::line_state::clamp_context;
pub use crate::text::clamp::line_state::clamp_cut;
pub use crate::text::clamp::line_state::clamp_lines_for;
pub use crate::text::clamp::line_state::clamp_next_seq;
pub use crate::text::clamp::line_state::clamp_para;
pub use crate::text::clamp::line_state::forget_clamp_buffers;
pub use crate::text::clamp::paragraph_rows::clamp_empty_probe;
pub use crate::text::clamp::paragraph_rows::clamp_probe;
pub use crate::text::clamp::paragraph_rows::publish_para_rows;
pub use crate::text::clamp::paragraph_rows::set_para_budget;
pub use crate::text::clamp::paragraph_rows::set_para_tag;
pub use crate::text::clamp::paragraph_rows::take_para_budget;
use crate::text::clamp::paragraph_rows::take_para_rows;
pub use crate::text::clamp::paragraph_rows::take_para_tag;

// owner: A

use crate::text::clamp::line_state::CLAMP_CUTS;

use crate::text::clamp::line_state::CLAMP_PARA;

use crate::text::clamp::paragraph_rows::PARA_ROWS;

use crate::text::clamp::paragraph_rows::PARA_TAG;

use crate::text::clamp::paragraph_rows::PARA_BUDGET;
