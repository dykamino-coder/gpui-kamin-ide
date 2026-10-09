//! Carry the nearest scrollport separately from ordinary block inline constraints.
use crate::render::ratio_basis;
use crate::computed::orthogonal::{AxisSizes, InlineConstraint, available};
use crate::computed::{Computed, Display, Overflow, Position};
use crate::value::Len;

fn px(value: Option<Len>) -> Option<f32> {
    match value {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    }
}

pub(crate) fn edges(style: &Computed, vertical: bool) -> f32 {
    let b = style.borders();
    let sides = if vertical {
        [b.top, b.bottom, style.padding.top, style.padding.bottom]
    } else {
        [b.left, b.right, style.padding.left, style.padding.right]
    };
    sides.into_iter().filter_map(px).sum()
}

pub(crate) fn axis(style: &Computed, vertical: bool) -> AxisSizes {
    let (size, min, max) = if vertical {
        (style.height, style.min_height, style.max_height)
    } else {
        (style.width, style.min_width, style.max_width)
    };
    let adjustment = if style.border_box == Some(true) {
        edges(style, vertical)
    } else {
        0.0
    };
    let inner = |value| px(value).map(|v| (v - adjustment).max(0.0));
    AxisSizes {
        size: inner(size),
        min: inner(min),
        max: inner(max),
    }
}

fn ordinary(style: &Computed) -> bool {
    matches!(style.display, None | Some(Display::Block))
        && !matches!(style.position, Some(Position::Absolute | Position::Fixed))
        && !style.float.is_some_and(|v| v != 0)
        && !style.ortho_col
}

pub(super) fn flow_constraint(style: &Computed) -> Option<InlineConstraint> {
    // Parallel table cells receive their inline measure from the column track.
    // A containing-block fallback must not override that intrinsic probe.
    (!style.ortho_col).then_some(style.orthogonal_inline).flatten()
}

pub(crate) fn resolve(
    style: &mut Computed,
    parent: &Computed,
    viewport: (f32, f32),
    principal: bool,
) {
    style.orthogonal_scrollport = if [style.overflow_x, style.overflow_y]
        .into_iter()
        .any(|v| matches!(v, Some(Overflow::Hidden | Overflow::Scroll)))
    {
        Some([axis(style, false), axis(style, true)])
    } else {
        parent.orthogonal_scrollport
    };
    let vertical = style.vertical == Some(true);
    let parent_vertical = parent.vertical == Some(true);
    style.orthogonal_inline = if vertical == parent_vertical && ordinary(style) {
        crate::render::orthogonal_fixed_child::containing_inline(parent)
            .map(|constraint| crate::render::orthogonal_fixed_child::inherit(style, constraint))
    } else {
        None
    };
    if principal || !vertical || parent_vertical || !ordinary(style) || !ordinary(parent) {
        return;
    }
    // Normal block flow may overflow its containing block. The flex column
    // used by the adapter must not compress this child's physical height.
    style.flex_shrink = Some(0.0);
    let parent_axis = ratio_basis::block_axis(parent);
    let fallback = available(
        parent_axis,
        parent.orthogonal_scrollport.map(|s| s[1]),
        viewport.1,
    );
    let basis = parent_axis
        .size
        .map(|v| parent_axis.clamp(v))
        .unwrap_or(fallback);
    let own_edges = edges(style, true);
    if let Some(Len::Pct(k)) = style.height {
        style.height = Some(Len::Px(k * basis));
    }
    let own = axis(style, true);
    if [style.overflow_x, style.overflow_y]
        .into_iter()
        .any(|v| matches!(v, Some(Overflow::Hidden | Overflow::Scroll)))
    {
        style.orthogonal_scrollport = Some([axis(style, false), own]);
    }
    let margin = |value| match value {
        Some(Len::Pct(k)) => axis(parent, false).size.unwrap_or(0.0) * k,
        other => px(other).unwrap_or(0.0),
    };
    let margins = margin(style.margin.top) + margin(style.margin.bottom);
    style.orthogonal_inline = Some(InlineConstraint {
        available: (fallback - own_edges - margins).max(0.0),
        fixed: own.size,
        min: own.min,
        max: own.max,
    });
    // A wrapping cap is not a percentage basis or a used inline size.
    style.ortho_limit = own.size.or(Some((fallback - own_edges - margins).max(0.0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parallel_table_column_uses_its_track_measure() {
        let mut style = Computed {
            ortho_col: true,
            orthogonal_inline: Some(InlineConstraint {
                available: 600.0, fixed: None, min: None, max: None,
            }),
            ..Computed::default()
        };
        assert_eq!(flow_constraint(&style), None);
        style.ortho_col = false;
        assert_eq!(flow_constraint(&style), style.orthogonal_inline);
    }
    #[test]
    fn percentage_uses_fallback_but_fixed_parent_keeps_its_own_basis() {
        let mut child = Computed {
            vertical: Some(true),
            height: Some(Len::Pct(0.5)),
            ..Computed::default()
        };
        resolve(&mut child, &Computed::default(), (800.0, 600.0), false);
        assert_eq!(child.height, Some(Len::Px(300.0)));
        let parent = Computed {
            height: Some(Len::Px(720.0)),
            ..Computed::default()
        };
        child.height = Some(Len::Pct(0.5));
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.height, Some(Len::Px(360.0)));
    }

    #[test]
    fn nearest_indefinite_scroller_replaces_an_outer_fixed_scroller() {
        let outer = Computed {
            orthogonal_scrollport: Some([
                AxisSizes::default(),
                AxisSizes {
                    size: Some(100.0),
                    ..AxisSizes::default()
                },
            ]),
            ..Computed::default()
        };
        let mut inner = Computed {
            overflow_y: Some(Overflow::Hidden),
            ..Computed::default()
        };
        resolve(&mut inner, &outer, (800.0, 600.0), false);
        assert_eq!(inner.orthogonal_scrollport.unwrap()[1].size, None);
        let mut child = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        resolve(&mut child, &inner, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline.unwrap().available, 600.0);
    }

    #[test]
    fn inherited_writing_mode_does_not_establish_another_orthogonal_boundary() {
        let c = InlineConstraint {
            available: 100.0,
            fixed: None,
            min: None,
            max: None,
        };
        let parent = Computed {
            vertical: Some(true),
            orthogonal_inline: Some(c),
            ..Computed::default()
        };
        let mut child = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline, Some(c));
        child.height = Some(Len::Px(40.0));
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline.unwrap().fixed, Some(40.0));
        child.display = Some(Display::Flex);
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline, None);
    }

    #[test]
    fn percentage_margins_use_cb_width_and_clip_does_not_replace_scrollport() {
        let mut parent = Computed {
            width: Some(Len::Px(500.0)),
            height: Some(Len::Px(200.0)),
            ..Computed::default()
        };
        let mut child = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        child.margin.top = Some(Len::Pct(0.1));
        child.margin.bottom = Some(Len::Pct(0.1));
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline.unwrap().available, 100.0);
        parent.border_box = Some(true);
        parent.padding.left = Some(Len::Px(10.0));
        parent.padding.right = Some(Len::Px(10.0));
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline.unwrap().available, 104.0);
        let mut clipped = Computed {
            overflow_y: Some(Overflow::Clip),
            ..Computed::default()
        };
        let outer = Computed {
            orthogonal_scrollport: Some([
                AxisSizes::default(),
                AxisSizes {
                    size: Some(200.0),
                    ..AxisSizes::default()
                },
            ]),
            ..Computed::default()
        };
        resolve(&mut clipped, &outer, (800.0, 600.0), false);
        assert_eq!(clipped.orthogonal_scrollport, outer.orthogonal_scrollport);
    }
}
