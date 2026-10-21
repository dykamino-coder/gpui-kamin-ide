//! Перевод `calc-size()` в стиль коробки GPUI: выражения размеров, их базис
//! (`CalcSizeBasis` → `CssSizingKeyword`) и бокс отсчёта; обычные ключевые слова
//! размеров этот этап не трогает.

use super::*;
use crate::style::computed::CalcSizeBasis;

pub(super) fn apply(mut d: Div, c: &Computed) -> Div {
    if c.calc_size
        .iter()
        .chain(c.calc_size_max.iter())
        .all(Option::is_none)
    {
        return d;
    }
    d.style().calc_size_content_box = Some(c.border_box != Some(true));
    d.style().calc_size = Some(c.calc_size);
    d.style().calc_size_max = Some(c.calc_size_max);
    d.style().calc_size_basis = Some(c.calc_size_basis.map(|basis| match basis {
        None | Some(CalcSizeBasis::Auto) => None,
        Some(CalcSizeBasis::MinContent) => Some(gpui::CssSizingKeyword::MinContent),
        Some(CalcSizeBasis::MaxContent) => Some(gpui::CssSizingKeyword::MaxContent),
        Some(CalcSizeBasis::FitContent) => Some(gpui::CssSizingKeyword::FitContent),
        Some(CalcSizeBasis::Stretch) => Some(gpui::CssSizingKeyword::Stretch),
    }));
    d
}
