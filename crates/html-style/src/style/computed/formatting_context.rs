//! Границы контекстов форматирования, целиком определяемые вычисленными значениями.

use crate::style::computed::{Computed, Display};

/// Whether these computed values establish an independent formatting context.
/// This predicate needs no DOM node, including when checking a containing block.
pub fn own_context_style(c: &Computed) -> bool {
    !matches!(
        c.reset_box_layout.overflow_y,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || !matches!(
        c.reset_box_layout.overflow_x,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || matches!(
        c.reset_box_layout.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::InlineBlock)
            | Some(Display::Table)
            | Some(Display::InlineTable)
    ) || matches!(
        c.reset_positioned.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) || c.reset_box_layout.float.is_some()
        || c.reset_context.contain_paint == Some(true)
        || c.reset_context.contain_layout == Some(true)
        || c.reset_context.flow_root == Some(true)
        // css-align-3 §align-block: не-`normal` `align-content` на блочном
        // контейнере — тот же `display: flow-root`, что пишет эталон
        // `align-content-block-001-ref`. Через край такой коробки поля не
        // схлопываются ни с детьми, ни насквозь.
        || c.reset_flex.align_content_block
        || matches!(c.reset_box_layout.display, Some(Display::TableCell))
        || c.reset_fragmentation.column_count.is_some()
        || c.reset_fragmentation.column_width.is_some()
        // css-multicol-1 §column-span: «The element establishes an independent
        // formatting context … When 'column-span' is 'all', it always does» —
        // и вне многоколоночника тоже. Поля детей спаннера с его полями не
        // схлопываются (`multicol-span-all-margin-nested-firstchild-001`).
        || c.reset_fragmentation.column_span == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_containment_keeps_the_surrounding_formatting_context() {
        let size = Computed {
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    contain_size: Some(true),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };
        assert!(!own_context_style(&size));
        let layout = {
            let style_base = size.clone();
            Computed {
                reset_context: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetContext {
                        contain_layout: Some(true),
                        ..(*style_base.reset_context).clone()
                    },
                ),
                ..style_base
            }
        };
        assert!(own_context_style(&layout));
        let paint = {
            let style_base = size;
            Computed {
                reset_context: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetContext {
                        contain_paint: Some(true),
                        ..(*style_base.reset_context).clone()
                    },
                ),
                ..style_base
            }
        };
        assert!(own_context_style(&paint));
    }
}
