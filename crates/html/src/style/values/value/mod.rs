//! Разбор значений CSS: длины, цвета, числа.
//!
//! Отдельный модуль, потому что одно и то же значение приходит из трёх мест —
//! `style=""`, правило в `<style>` и значение по умолчанию тега, — и разбирать
//! его надо одинаково.

#[path = "calc_sum.rs"]
mod calc_sum;
#[cfg(test)]
mod tests;
pub use calc_sum::Sum;
#[cfg(test)]
mod calc_tests;

mod anchor;
mod calc;
mod calc_pool;
mod color;
mod color_channels;
mod color_names;
mod len;
mod len_parse;
mod root_metrics;
pub use anchor::{AnchorFn, AnchorSide, AnchorSize, anchor_get, anchor_store};
use anchor::{parse_anchor, parse_anchor_calc, parse_anchor_minmax};
use calc::{Calc, Val, eval_calc, parse_calc};
use calc_pool::fold_simple_calc;
pub use calc_pool::{
    calc_get, calc_pct_px, calc_reset, calc_store, fold_zero_percentage, lerp_len, number,
};
pub use color::{Color, dark_scheme, set_dark_scheme};
use color_names::named;
pub use len::Len;
pub(crate) use len::css_number;
use root_metrics::ROOT_FONT_VIEW;
pub use root_metrics::{
    reset_root_metrics, root_font_px, root_line_px, set_root_font_view, set_root_metrics,
};
