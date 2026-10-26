//! Границы контекстов форматирования, целиком определяемые вычисленными значениями.

use crate::style::computed::{Computed, Display};

/// Whether these computed values establish an independent formatting context.
/// This predicate needs no DOM node, including when checking a containing block.
pub(crate) fn own_context_style(c: &Computed) -> bool {
    !matches!(
        c.overflow_y,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || !matches!(
        c.overflow_x,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || matches!(
        c.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::InlineBlock)
            | Some(Display::Table)
            | Some(Display::InlineTable)
    ) || matches!(
        c.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) || c.float.is_some()
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
        || c.flow_root == Some(true)
        // css-align-3 §align-block: не-`normal` `align-content` на блочном
        // контейнере — тот же `display: flow-root`, что пишет эталон
        // `align-content-block-001-ref`. Через край такой коробки поля не
        // схлопываются ни с детьми, ни насквозь.
        || c.align_content_block
        || matches!(c.display, Some(Display::TableCell))
        || c.column_count.is_some()
        || c.column_width.is_some()
        // css-multicol-1 §column-span: «The element establishes an independent
        // formatting context … When 'column-span' is 'all', it always does» —
        // и вне многоколоночника тоже. Поля детей спаннера с его полями не
        // схлопываются (`multicol-span-all-margin-nested-firstchild-001`).
        || c.column_span == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_containment_keeps_the_surrounding_formatting_context() {
        let size = Computed {
            contain_size: Some(true),
            ..Default::default()
        };
        assert!(!own_context_style(&size));
        let layout = Computed {
            contain_layout: Some(true),
            ..size.clone()
        };
        assert!(own_context_style(&layout));
        let paint = Computed {
            contain_paint: Some(true),
            ..size
        };
        assert!(own_context_style(&paint));
    }
}
