//! Предикаты `clear` для раскладки флоатов (CSS 2.1 §9.5.2): снятие `clear` в дереве без
//! единого флоата (флаг контекста — клиренса там нет, и поля обязаны схлопнуться),
//! коробка своего контекста, которой заведомо нет места рядом с флоатами (`bfc_no_fit`,
//! §9.5 последний абзац).

use crate::dom::Element;
use crate::layout::box_kind::{in_flow, inline_marked_block, own_context};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_missing_zero};
use crate::tolerance::COORD_EPS;

/// CSS2 §9.5.2: свойство clear не создаёт clearance без флоатов.
/// Флаг относится к текущему документу, включая отдельный контекст iframe.
/// Исходный computed style остаётся доступен наследованию и CSSOM.
pub(crate) fn effective_clear(style: &Computed, values: crate::values::ValueCx<'_>) -> Option<i8> {
    style
        .reset_box_layout
        .clear
        .filter(|_| values.clearance_possible)
}

/// §9.5, последний абзац: коробка своего контекста, которой РЯДОМ с флоатами
/// заведомо нет места, — «implementations should clear the said element by
/// placing it below any preceding floats». `float_min_w` — нижняя оценка
/// ширины УЗЧАЙШЕГО флоата: на любой полосе с флоатом свободно не больше
/// `cb − float_min_w`. Содержащий блок `cb` — ширина ближайшего предка в
/// точках (`BoxConstraints::cb_width`): это верхняя оценка настоящей ширины,
/// поэтому «не влезает» при ней не влезает и в настоящую.
pub(crate) fn bfc_no_fit(n: &Element, float_min_w: f32, cb: Option<f32>) -> bool {
    let Some(cb) = cb else {
        return false;
    };
    let Some(Len::Px(w)) = n.style.used().reset_box_layout.width else {
        return false;
    };
    let b = n.style.used().borders();
    let bw = if n.style.used().reset_border.border_box == Some(true) {
        Some(w)
    } else {
        match (
            px_missing_zero(n.style.used().reset_box_layout.padding.left),
            px_missing_zero(n.style.used().reset_box_layout.padding.right),
            px_missing_zero(b.left),
            px_missing_zero(b.right),
        ) {
            (Some(pl), Some(pr), Some(bl), Some(br)) => Some(w + pl + pr + bl + br),
            _ => None,
        }
    };
    (!n.inline || inline_marked_block(n))
        && in_flow(&n.style.used())
        && matches!(
            n.style.used().reset_box_layout.display,
            None | Some(Display::Block) | Some(Display::Flex) | Some(Display::Grid)
        )
        && own_context(n)
        && bw.is_some_and(|bw| bw + float_min_w > cb + COORD_EPS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::block::margins::MarginCx;
    use crate::layout::block::struts::through_strut;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn inert_clear_does_not_split_margin_struts_or_mutate_computed_style() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut e = anon_element("div", vec![]);
        e.style.used_mut().reset_box_layout.clear = Some(0);
        e.style.used_mut().reset_context.clear_inherit = true;
        e.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(12.));
        let active = doc.values();
        assert_eq!(effective_clear(&e.style.used(), active), Some(0));
        assert!(through_strut(&e, MarginCx::new(active)).is_none());
        let mut inert = active;
        inert.clearance_possible = false;
        assert_eq!(effective_clear(&e.style.used(), inert), None);
        assert_eq!(through_strut(&e, MarginCx::new(inert)), Some((12., 0.)));
        assert!(!inert.keep().vcx().clearance_possible);
        assert_eq!(e.style.used().reset_box_layout.clear, Some(0));
        assert!(e.style.used().reset_context.clear_inherit);
    }
}
