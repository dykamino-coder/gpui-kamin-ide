//! Percentages in a vertical containing block refer to its INLINE size, its physical height.
//!
//! CSS 2.1 §8.3/§8.4 and css-box-4: percentage margins and padding refer to the inline size of
//! the containing block; css-writing-modes-4 §7.1 maps a percentage `height` of a box in vertical
//! flow onto that same inline axis. Like a horizontal block's width, a vertical block's height is
//! definite when it fills its parent's inline axis (CSS 2.1 §10.3.3 in logical terms), all the way
//! up to the initial containing block (Blink `ComputeInlineSizeForFragment` / fill-available;
//! Servo `containing_block.size.inline`). taffy resolves these percentages against the PHYSICAL
//! width and treats a stretched height as indefinite, so they are resolved here, once the parent's
//! content inline size is known.
use crate::style::computed::{Computed, Display, Position, Sides};
use crate::style::values::value::Len;

fn px(value: Option<Len>) -> Option<f32> {
    match value {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        _ => None,
    }
}

fn out_of_flow(style: &Computed) -> bool {
    matches!(style.position, Some(Position::Absolute | Position::Fixed))
}

/// Resolve the box's own inline-axis percentages against its vertical containing block.
pub(crate) fn resolve_percentages(style: &mut Computed, parent: &Computed) {
    let Some(cb) = parent.inline_definite else {
        return;
    };
    if parent.vertical != Some(true) || out_of_flow(style) {
        return;
    }
    let at = |value: &mut Option<Len>| {
        if let Some(Len::Pct(k)) = *value {
            *value = Some(Len::Px(k * cb));
        }
    };
    at(&mut style.height);
    at(&mut style.min_height);
    at(&mut style.max_height);
    for sides in [&mut style.margin, &mut style.padding] {
        let Sides {
            top,
            right,
            bottom,
            left,
        } = sides;
        for side in [top, right, bottom, left] {
            at(side);
        }
    }
}

/// The box's own content inline size when it is definite and its inline axis is vertical.
pub(crate) fn own(style: &Computed, parent: &Computed, root: bool, icb_height: f32) -> Option<f32> {
    if style.vertical != Some(true) || style.contains_height() {
        return None;
    }
    let b = style.borders();
    let edges = px(b.top)? + px(b.bottom)? + px(style.padding.top)? + px(style.padding.bottom)?;
    match style.height {
        Some(Len::Px(h)) => {
            return Some(if style.border_box == Some(true) {
                (h - edges).max(0.0)
            } else {
                h
            });
        }
        None | Some(Len::Auto) => {}
        _ => return None,
    }
    // An automatic inline size fills the parent's content inline size only for an in-flow
    // block-level box of the same (parallel) flow; floats, atomic inlines and out-of-flow
    // boxes shrink to fit instead.
    if out_of_flow(style)
        || style.float.is_some_and(|side| side != 0)
        || !matches!(
            style.display,
            None | Some(Display::Block | Display::ListItem)
        )
    {
        return None;
    }
    let space = if root {
        Some(icb_height)
    } else if parent.vertical == Some(true) {
        parent.inline_definite
    } else {
        None
    }?;
    let margins = px(style.margin.top)? + px(style.margin.bottom)?;
    Some((space - margins - edges).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertical() -> Computed {
        Computed {
            vertical: Some(true),
            ..Computed::default()
        }
    }

    #[test]
    fn stretched_vertical_blocks_carry_the_icb_height_down() {
        let mut html = vertical();
        html.inline_definite = own(&html, &Computed::default(), true, 600.0);
        assert_eq!(html.inline_definite, Some(600.0));
        let mut body = vertical();
        body.margin.top = Some(Len::Px(8.0));
        body.margin.bottom = Some(Len::Px(8.0));
        assert_eq!(own(&body, &html, false, 600.0), Some(584.0));
        let mut float = vertical();
        float.float = Some(-1);
        assert_eq!(own(&float, &html, false, 600.0), None);
    }

    #[test]
    fn percentages_refer_to_the_vertical_inline_size() {
        let mut parent = vertical();
        parent.inline_definite = Some(200.0);
        let mut child = vertical();
        child.height = Some(Len::Pct(0.25));
        child.padding.left = Some(Len::Pct(0.5));
        resolve_percentages(&mut child, &parent);
        assert_eq!(child.height, Some(Len::Px(50.0)));
        assert_eq!(child.padding.left, Some(Len::Px(100.0)));
        let mut abs = vertical();
        abs.position = Some(Position::Absolute);
        abs.height = Some(Len::Pct(0.25));
        resolve_percentages(&mut abs, &parent);
        assert_eq!(abs.height, Some(Len::Pct(0.25)));
    }
}
