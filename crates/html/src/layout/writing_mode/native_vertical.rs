//! Native vertical paragraphs retain orthogonal flow, absolute and table-track sizing contracts.

use crate::computed::Computed;
use crate::computed::orthogonal::{InlineConstraint, InlineKeyword};
use crate::value::Len;

/// Float placement removes the CSS float flag; preserve its intrinsic sizing contract.
pub(crate) fn claim_float_inline_size(
    style: &mut Computed,
    parent: &Computed,
    children: &[crate::dom::Node],
) {
    // CSS Containment 2 §3.1: a float's shrink-to-fit width must measure
    // the empty containment box even after placement removes its float flag.
    if style.contains_width() && matches!(style.width, None | Some(Len::Auto)) {
        style.width = Some(Len::FitContent);
    }
    // Native block children already establish the float's inline size and stretch together.
    // Only the rotated inline paragraph needs an explicit outer size claim.
    if children.iter().any(|node| {
        matches!(node, crate::dom::Node::Element(child)
        if !crate::render::inline_level(child) && crate::render::in_flow(&child.style))
    }) {
        return;
    }
    if style.vertical.or(parent.vertical) == Some(true)
        && matches!(style.height, None | Some(Len::Auto))
    {
        // The float's automatic orthogonal inline size is fit-content. Resolve
        // that size at its flow root before stretching parallel block children;
        // propagating auto claims would give each column its own min-content height.
        style.height = Some(Len::FitContent);
        style.hug_inline = true;
        style.hug_claim = true;
    }
}

pub(crate) fn constraint(style: &Computed, fallback: f32) -> Option<InlineConstraint> {
    crate::render::vertical_intrinsic::constraint(style, fallback)
        .map(|(_, value)| value)
        .or_else(|| {
            let own = crate::render::orthogonal_inline::axis(style, true);
            own.size.map(|size| InlineConstraint {
                available: fallback,
                fixed: Some(size),
                min: own.min,
                max: own.max,
            })
        })
        .or(crate::render::orthogonal_inline::flow_constraint(style))
        .or_else(|| crate::render::orthogonal_absolute::constraint(style, fallback))
        .or_else(|| {
            if !style.hug_inline {
                return None;
            }
            let own = crate::render::orthogonal_inline::axis(style, true);
            Some(InlineConstraint {
                available: fallback,
                fixed: own.size,
                min: own.min,
                max: own.max,
            })
        })
        .or_else(|| {
            // The former rotated wrapper explicitly requested min-content for
            // parallel table columns; available space alone must not replace it.
            if !style.ortho_col || style.ortho_limit.is_none() {
                return None;
            }
            let own = crate::render::orthogonal_inline::axis(style, true);
            Some(InlineConstraint {
                available: fallback,
                fixed: own.size,
                min: own.min,
                max: own.max,
            })
        })
}

pub(crate) fn keyword(style: &Computed) -> Option<InlineKeyword> {
    match style.height {
        Some(Len::MinContent) => Some(InlineKeyword::MinContent),
        Some(Len::MaxContent) => Some(InlineKeyword::MaxContent),
        Some(Len::FitContent) => Some(InlineKeyword::FitContent),
        _ => None,
    }
    .or_else(|| {
        (style.ortho_col && style.ortho_limit.is_some()).then_some(InlineKeyword::MinContent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_claim_follows_own_writing_mode_and_preserves_definite_inline_sizes() {
        let parent = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        let mut child = Computed::default();
        claim_float_inline_size(&mut child, &parent, &[]);
        assert!(child.hug_inline && child.hug_claim);
        assert_eq!(child.height, Some(Len::FitContent));
        let mut horizontal = Computed {
            vertical: Some(false),
            ..Computed::default()
        };
        claim_float_inline_size(&mut horizontal, &parent, &[]);
        assert!(!horizontal.hug_inline);
        let mut fixed = Computed {
            height: Some(Len::Px(70.0)),
            ..Computed::default()
        };
        claim_float_inline_size(&mut fixed, &parent, &[]);
        assert!(!fixed.hug_inline);
    }

    #[test]
    fn auto_shrink_to_fit_claim_uses_real_intrinsics_and_own_limits() {
        let style = Computed {
            vertical: Some(true),
            hug_inline: true,
            min_height: Some(Len::Px(80.0)),
            max_height: Some(Len::Px(250.0)),
            ..Computed::default()
        };
        let value = constraint(&style, 600.0).unwrap();
        assert_eq!(value.used_keyword(None, 40.0, 180.0), 180.0);
        assert_eq!(value.used_keyword(None, 40.0, 900.0), 250.0);
        assert_eq!(value.used_keyword(None, 40.0, 60.0), 80.0);
    }

    #[test]
    fn authored_inline_size_remains_definite_without_an_orthogonal_boundary() {
        let style = Computed {
            vertical: Some(true),
            display: Some(crate::computed::Display::InlineBlock),
            height: Some(Len::Px(160.0)),
            ..Computed::default()
        };
        let value = constraint(&style, 600.0).unwrap();
        assert_eq!(value.fixed, Some(160.0));
        assert_eq!(
            value.used_keyword(Some(InlineKeyword::MinContent), 40.0, 700.0),
            160.0
        );
    }

    #[test]
    fn parallel_table_track_keeps_min_content_and_own_limits() {
        let mut style = Computed {
            vertical: Some(true),
            ortho_col: true,
            ortho_limit: Some(200.0),
            min_height: Some(Len::Px(40.0)),
            ..Computed::default()
        };
        let value = constraint(&style, 200.0).unwrap();
        assert_eq!(value.used_keyword(keyword(&style), 30.0, 900.0), 40.0);
        style.height = Some(Len::Px(80.0));
        assert_eq!(
            constraint(&style, 200.0)
                .unwrap()
                .used_keyword(keyword(&style), 30.0, 900.0),
            80.0
        );
    }

    #[test]
    fn ordinary_intrinsic_height_does_not_become_a_table_minimum() {
        let style = Computed {
            vertical: Some(true),
            height: Some(Len::MaxContent),
            ..Computed::default()
        };
        let value = constraint(&style, 200.0).unwrap();
        assert_eq!(value.used_keyword(keyword(&style), 30.0, 900.0), 900.0);
        let intrinsic_table = Computed {
            ortho_col: true,
            ortho_limit: Some(200.0),
            ..style
        };
        assert_eq!(
            constraint(&intrinsic_table, 200.0).unwrap().used_keyword(
                keyword(&intrinsic_table),
                30.0,
                900.0
            ),
            900.0
        );
        let table = Computed {
            ortho_col: true,
            ..Computed::default()
        };
        assert!(constraint(&table, 200.0).is_none());
    }
}
