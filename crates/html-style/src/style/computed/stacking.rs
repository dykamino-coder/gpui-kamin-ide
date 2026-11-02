//! Принадлежность к контексту наложения, определяемая вычисленными CSS-значениями.

use crate::style::computed::{BoxStyle, Computed};

/// Stacking contexts isolate descendant paint order (CSS2 Appendix E).
pub fn stacking_context(c: &Computed) -> bool {
    // CSS Will Change §2.1; CSS Containment 2 §§3.2/3.3 also create contexts.
    c.reset_effects.will_change & crate::style::computed::wc::STACK != 0
        || c.reset_context.contain_layout == Some(true)
        || c.reset_context.contain_paint == Some(true)
        || c.reset_transforms.transform.is_some()
        // css-transforms-2 §transform-style-property: `preserve-3d` establishes
        // a stacking context (`transform-style-stacking-context`).
        || c.reset_transforms.preserve_3d == Some(true)
        || c.reset_transforms.translate.is_some()
        || c.reset_effects.opacity.is_some_and(|o| o < 1.0)
        // Compositing 1 §mix-blend-mode: non-normal blending creates a context.
        || c.reset_effects.blend.is_some_and(|b| b != 0)
        || c.reset_effects.isolate == Some(true)
        || c.reset_effects.filter.is_some()
        || (c.reset_positioned.z_index.is_some()
            && matches!(
                c.reset_positioned.position,
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
pub fn nested_context(parent: &BoxStyle) -> bool {
    parent.sc_ancestor
        || (!parent.reset_context.root_box
            && (stacking_context(parent)
                || (parent.reset_positioned.z_index.is_some() && parent.context.parent_flex_grid)
                || matches!(
                    parent.reset_positioned.position,
                    Some(crate::style::computed::Position::Fixed)
                        | Some(crate::style::computed::Position::Sticky)
                )))
}

/// Коробка — содержащий блок и для `position: fixed`: тот же список, что
/// барьер `under_tf` (`style::cascade::inherit`, `transform_ancestor`).
pub fn fixed_cb_layer_box(c: &Computed) -> bool {
    // css-transforms-2 §backface-visibility: `hidden` у участника 3D-контекста
    // — содержащий блок для всех потомков (`backface-visibility-hidden-004`).
    (c.reset_transforms.backface_hidden == Some(true) && c.reset_transforms.transform_ancestor)
        || c.reset_transforms.transform.is_some()
        || c.reset_transforms.preserve_3d == Some(true)
        || c.reset_context.contain_layout == Some(true)
        || c.reset_context.contain_paint == Some(true)
        || crate::style::computed::filter_contains_fixed(c)
        || c.reset_effects.will_change & crate::style::computed::wc::CB_FIXED != 0
}
