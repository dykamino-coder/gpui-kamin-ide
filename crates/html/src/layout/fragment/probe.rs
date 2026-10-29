//! Предикаты поддерева для фрагментации: монолит `contain: size` (`size_monolith`; Blink
//! `LayoutBox::IsMonolithic`) и коробки, внутри которых принудительные разрывы фрагментации
//! не видны (`forced_opaque`; css-break-4 §possible-breaks).

use crate::dom::Element;
use crate::style::computed::Display;

/// `contain: size` — монолит везде, где есть фрагментация (Blink
/// `LayoutBox::IsMonolithic`, `layout_box.cc`: `ShouldApplySizeContainment()`
/// = `StyleRef().ContainsSize() && IsEligibleForSizeContainment()`; обе оси —
/// `size`/`strict`, не `inline-size`; `contain-intrinsic-size` не участвует;
/// таблица, группа, ряд и ячейка не годны — `layout_table*.h`
/// `IsEligibleForSizeContainment() … return false`). `content-visibility:
/// hidden` ставит тот же `contain_size`.
pub(crate) fn size_monolith(k: &Element) -> bool {
    k.style.used().contains_block_size()
        && !matches!(
            k.style.used().display,
            Some(Display::TableCell) | Some(Display::TableRow) | Some(Display::TableRowGroup)
        )
        && !matches!(
            k.tag.as_str(),
            "td" | "th" | "tr" | "thead" | "tbody" | "tfoot"
        )
}

pub(crate) fn forced_opaque(k: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    size_monolith(k)
        || ((scrolls(k.style.used().overflow_x) || scrolls(k.style.used().overflow_y))
            && !matches!(
                k.style.used().display,
                Some(Display::TableRow) | Some(Display::TableRowGroup)
            )
            && !matches!(k.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot"))
}

#[cfg(feature = "wpt-harness")]
pub(crate) mod diagnostics;
