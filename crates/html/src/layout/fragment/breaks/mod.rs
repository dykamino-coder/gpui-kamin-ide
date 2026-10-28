//! Принудительные разрывы и запреты разрыва при фрагментации: `break-before/after` и
//! `avoid` на краях коробки (`edges`) и монолиты стопки страниц (`page_monolith`,
//! css-break-4 §4.1; Blink `IsMonolithic`). Листы под внепоточные корни добавляет
//! план OOF стопки страниц.

use crate::dom::Element;
use crate::style::computed::Display;
mod edges;
pub(crate) use edges::edge_avoid;
pub(crate) use edges::edge_break;

/// Монолит стопки страниц (css-break-4 §4.1; Blink `IsMonolithic`):
/// замещаемый, прокручиваемый, `break-inside: avoid`, `contain: size`,
/// атомарный строчный. Сплошной строчный набор монолитом НЕ считается:
/// страница режет его по краю, а обе стороны пары режутся одинаково.
pub(crate) fn page_monolith(e: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    e.style.used().break_inside_avoid
        || e.style.used().contain_size == Some(true)
        || scrolls(e.style.used().overflow_x)
        || scrolls(e.style.used().overflow_y)
        || matches!(
            e.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            e.style.used().display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}
