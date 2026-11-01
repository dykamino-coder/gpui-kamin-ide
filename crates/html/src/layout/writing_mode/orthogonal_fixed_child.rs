//! Определённый вертикальный содержащий блок передаёт свой строчный размер содержимого
//! параллельным блочным детям: ограничение наследуется вниз с вычетом рамок, отбивок и
//! полей каждого ребёнка, проценты краёв и высоты решаются от этой базы, а неопределённый
//! родитель определённым содержащим блоком не становится.
use crate::layout::writing_mode::orthogonal_inline::{axis, edges};
use crate::style::computed::orthogonal::{AxisSizes, InlineConstraint};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};

pub(crate) fn normal_block_flow(child: &Computed, parent: &Computed) -> bool {
    !parent.wm().vertical()
        && matches!(parent.reset_box_layout.display, None | Some(Display::Block))
        && matches!(child.reset_box_layout.display, None | Some(Display::Block))
}

pub(super) fn containing_inline(parent: &Computed) -> Option<InlineConstraint> {
    // A parallel block fills its containing block's definite logical inline
    // size even while the parent's block size is being probed intrinsically.
    // Inline-blocks and table-cell flow roots also establish this containing
    // block; a wrapping fallback alone must not become a definite size.
    if parent.wm().vertical()
        && matches!(
            parent.reset_box_layout.display,
            None | Some(
                Display::Block | Display::InlineBlock | Display::ListItem | Display::TableCell
            )
        )
    {
        let inline = axis(parent, true);
        if let Some(size) = inline.size {
            let size = inline.clamp(size);
            return Some(InlineConstraint {
                available: size,
                fixed: Some(size),
                min: None,
                max: None,
            });
        }
    }
    parent.reset_context.orthogonal_inline
}

pub(super) fn inherit(style: &mut Computed, mut parent: InlineConstraint) -> InlineConstraint {
    let used_parent = parent.fixed.map(|fixed| {
        AxisSizes {
            size: None,
            min: parent.min,
            max: parent.max,
        }
        .clamp(fixed)
    });
    if let Some(basis) = used_parent {
        // Percentage edges use the containing block's logical inline size.
        let resolve = |value: &mut Option<Len>| {
            if let Some(Len::Pct(k)) = *value {
                *value = Some(Len::Px(k * basis));
            }
        };
        for style_field_index in 0..11 {
            let value = match style_field_index {
                0 => &mut style.reset_box_layout.height,
                1 => &mut style.reset_box_layout.min_height,
                2 => &mut style.reset_box_layout.max_height,
                3 => &mut style.reset_box_layout.margin.top,
                4 => &mut style.reset_box_layout.margin.right,
                5 => &mut style.reset_box_layout.margin.bottom,
                6 => &mut style.reset_box_layout.margin.left,
                7 => &mut style.reset_box_layout.padding.top,
                8 => &mut style.reset_box_layout.padding.right,
                9 => &mut style.reset_box_layout.padding.bottom,
                _ => &mut style.reset_box_layout.padding.left,
            };
            resolve(value);
        }
    }
    let own = axis(style, true);
    if let Some(size) = own.size {
        return InlineConstraint {
            available: size,
            fixed: Some(size),
            min: own.min,
            max: own.max,
        };
    }
    if let Some(size) = used_parent {
        let margin = |value| px_lossy_zero(value);
        let content = (size
            - edges(style, true)
            - margin(style.reset_box_layout.margin.top)
            - margin(style.reset_box_layout.margin.bottom))
        .max(0.0);
        parent = InlineConstraint {
            available: content,
            fixed: Some(content),
            min: own.min,
            max: own.max,
        };
    }
    // An indefinite parent's available space is not its intrinsic used size.
    parent
}

#[cfg(test)]
mod tests;
