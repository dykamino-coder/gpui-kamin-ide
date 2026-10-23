//! Таблицы и фрагментация: `border-collapse`, `border-spacing`,
//! `caption-side`, `empty-cells` (CSS 2.1 §17.4.1, §17.6, css-tables-3),
//! `orphans`, `widows` (css-break-3 §3.3).

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::LonghandId;
use crate::style::values::value::Len;

/// `<length [0,∞]>` (без долей и слов `<line-width>`).
fn spacing_length(v: &str, vcx: ValueCx<'_>) -> Option<Len> {
    // Отрицательная размерность негодна; `calc()` зажимается при вычислении.
    if v.trim_start().starts_with('-') {
        return None;
    }
    match Len::parse(v, vcx)? {
        Len::Pct(_) | Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
        Len::Px(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n) if n < 0.0 => None,
        l => Some(l),
    }
}

/// `border-spacing: <length [0,∞]>{1,2}`: одно значение — обе оси.
pub(crate) fn border_spacing(v: &str, vcx: ValueCx<'_>) -> Option<(Len, Len)> {
    match crate::style::computed::split_outside_parens(v).as_slice() {
        [both] => spacing_length(both, vcx).map(|l| (l, l)),
        [h, v2] => Some((spacing_length(h, vcx)?, spacing_length(v2, vcx)?)),
        _ => None,
    }
}

/// Пара ключевых слов: `true` — второе.
fn keyword_pair(v: &str, no: &str, yes: &str) -> Option<bool> {
    let lower = v.to_ascii_lowercase();
    (lower == no || lower == yes).then_some(lower == yes)
}

/// `orphans`, `widows`: `<integer [1,∞]>`.
pub(crate) fn line_count(v: &str) -> Option<u16> {
    v.parse::<u16>().ok().filter(|n| *n >= 1)
}

pub(super) fn accepts(id: LonghandId, v: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    match id {
        BorderCollapse => keyword_pair(v, "separate", "collapse").is_some(),
        BorderSpacing => border_spacing(v, vcx).is_some(),
        CaptionSide => keyword_pair(v, "top", "bottom").is_some(),
        EmptyCells => keyword_pair(v, "show", "hide").is_some(),
        Orphans | Widows => line_count(v).is_some(),
        _ => false,
    }
}

impl Computed {
    pub(super) fn apply_table(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        use LonghandId::*;
        match id {
            BorderCollapse => {
                self.border_collapse =
                    keyword_pair(v, "separate", "collapse").or(self.border_collapse)
            }
            BorderSpacing => {
                if let Some((h, v)) = border_spacing(v, vcx) {
                    self.border_spacing = Some((Some(h), Some(v)));
                }
            }
            CaptionSide => {
                self.caption_bottom = keyword_pair(v, "top", "bottom").or(self.caption_bottom)
            }
            EmptyCells => {
                self.empty_cells_hide = keyword_pair(v, "show", "hide").or(self.empty_cells_hide)
            }
            Orphans => self.orphans = line_count(v).or(self.orphans),
            Widows => self.widows = line_count(v).or(self.widows),
            _ => {}
        }
    }

    pub(super) fn inherit_table(&mut self, id: LonghandId) {
        use LonghandId::*;
        match id {
            BorderCollapse => self.border_collapse = None,
            BorderSpacing => self.border_spacing = None,
            CaptionSide => self.caption_bottom = None,
            EmptyCells => self.empty_cells_hide = None,
            Orphans => self.orphans = None,
            Widows => self.widows = None,
            _ => {}
        }
    }
}
