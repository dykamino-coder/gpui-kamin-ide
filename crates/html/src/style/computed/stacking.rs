//! Stacking-context membership determined by computed CSS values.

use crate::style::computed::Computed;

/// Stacking contexts isolate descendant paint order (CSS2 Appendix E).
pub(crate) fn stacking_context(c: &Computed) -> bool {
    // CSS Will Change §2.1; CSS Containment 2 §§3.2/3.3 also create contexts.
    c.will_change & crate::style::computed::wc::STACK != 0
        || c.contain_layout == Some(true)
        || c.contain_paint == Some(true)
        || c.transform.is_some()
        // css-transforms-2 §transform-style-property: `preserve-3d` establishes
        // a stacking context (`transform-style-stacking-context`).
        || c.preserve_3d == Some(true)
        || c.translate.is_some()
        || c.opacity.is_some_and(|o| o < 1.0)
        // Compositing 1 §mix-blend-mode: non-normal blending creates a context.
        || c.blend.is_some_and(|b| b != 0)
        || c.isolate == Some(true)
        || c.filter.is_some()
        || (c.z_index.is_some()
            && matches!(
                c.position,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
                    | Some(crate::style::computed::Position::Sticky)
            ))
}

/// Лежит ли ребёнок коробки `parent` внутри НЕКОРНЕВОГО контекста наложения.
///
/// CSS 2.1 прил. E: отрицательный `z-index` красится шагом 3 БЛИЖАЙШЕГО
/// контекста — сразу после его фона и рамки (Blink `PaintLayerPainter`:
/// `kSelfBlockBackgroundOnly`, затем `kNegativeZOrderChildren`). Если такой
/// контекст — корень, коробка ложится под весь поток документа, даже под фон
/// позиционированного без `z-index` родителя (`fixed-pos-stacking-001`).
/// Корневой элемент (`root_box`) контекст образует всегда, но корневой.
/// `fixed`/`sticky` образуют контекст и с `z-index: auto` (css-position-3 §3).
pub(crate) fn nested_context(parent: &Computed) -> bool {
    parent.sc_ancestor
        || (!parent.root_box
            && (stacking_context(parent)
                || (parent.z_index.is_some() && parent.parent_flex_grid)
                || matches!(
                    parent.position,
                    Some(crate::style::computed::Position::Fixed)
                        | Some(crate::style::computed::Position::Sticky)
                )))
}
