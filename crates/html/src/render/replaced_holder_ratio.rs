//! Expose a replaced image's ratio on its layout holder for deferred automatic sizing.
//! The inner measured image alone cannot transfer the holder's opposite-axis bounds.

use crate::computed::Computed;
use crate::value::Len;
use gpui::{Div, Styled};

pub(super) fn apply(holder: &mut Div, style: &Computed, ratio: Option<f32>, fixed: bool) -> bool {
    let Some(ratio) = ratio.filter(|r| r.is_finite() && *r > 0.0) else {
        return false;
    };
    if fixed {
        return false;
    }
    let automatic = |length| matches!(length, None | Some(Len::Auto));
    let (width_auto, height_auto) = (automatic(style.width), automatic(style.height));
    if width_auto == height_auto {
        // Both-auto flex images retain their intrinsic measured contribution.
        // Changing it regressed flex-aspect-ratio-027/028 in the prior comparison.
        return false;
    }
    let percentage =
        matches!(style.width, Some(Len::Pct(_))) || matches!(style.height, Some(Len::Pct(_)));
    if percentage {
        if style.aspect_ratio.is_none() && !natural_content_box(holder, style) {
            return false;
        }
        holder.style().aspect_ratio = Some(ratio);
        // Keep the inner image's object-fit and independently measured natural size.
        return false;
    }
    if style.flex_item {
        holder.style().aspect_ratio = Some(ratio);
        return true;
    }
    false
}

/// Natural ratios use content coordinates even when authored dimensions are border-box.
/// Rebuild raw numeric bounds rather than retaining apply's flattened padding offsets.
fn natural_content_box(holder: &mut Div, style: &Computed) -> bool {
    use gpui::{DefiniteLength, Length};
    let border = style.borders();
    let edge = |sides: [Option<Len>; 4]| -> Option<f32> {
        sides.into_iter().try_fold(0.0, |sum, side| match side {
            None => Some(sum),
            Some(Len::Px(value)) => Some(sum + value),
            _ => None,
        })
    };
    let offsets = if style.border_box == Some(true) {
        let Some(x) = edge([
            style.padding.left,
            style.padding.right,
            border.left,
            border.right,
        ]) else {
            return false;
        };
        let Some(y) = edge([
            style.padding.top,
            style.padding.bottom,
            border.top,
            border.bottom,
        ]) else {
            return false;
        };
        [x, y]
    } else {
        [0.0, 0.0]
    };
    let values = [
        style.width,
        style.height,
        style.min_width,
        style.min_height,
        style.max_width,
        style.max_height,
    ];
    let mut converted: [Option<Length>; 6] = [None; 6];
    for (index, value) in values.into_iter().enumerate() {
        let offset = offsets[index % 2];
        let percentage = match value {
            Some(Len::Pct(_)) => true,
            Some(Len::Calc(index)) => crate::value::calc_get(index).pct != 0.0,
            _ => false,
        };
        if percentage
            && index % 2 == 1
            && !style.cb_height_def
            && !style.root_box
            && !matches!(
                style.position,
                Some(crate::computed::Position::Absolute | crate::computed::Position::Fixed)
            )
        {
            // Preserve apply's unresolved block-percentage behavior for min/max too.
            continue;
        }
        converted[index] = match value {
            None | Some(Len::Auto) => None,
            Some(Len::MinContent | Len::MaxContent | Len::FitContent) => return false,
            Some(value) if offset == 0.0 => Some(crate::apply::len_to_gpui(value).into()),
            Some(Len::Px(value)) => Some(gpui::px((value - offset).max(0.0)).into()),
            Some(Len::Pct(value)) => Some(DefiniteLength::Calc(-offset, value).into()),
            Some(Len::Calc(index)) => {
                let Some((percentage, pixels)) = crate::value::calc_get(index).pct_px() else {
                    return false;
                };
                Some(DefiniteLength::Calc(pixels - offset, percentage).into())
            }
            _ => return false,
        };
    }
    let native = holder.style();
    native.content_box = Some(true);
    native.size.width = converted[0];
    native.size.height = converted[1];
    native.min_size.width = converted[2];
    native.min_size.height = converted[3];
    native.max_size.width = converted[4];
    native.max_size.height = converted[5];
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{DefiniteLength, div, px};

    #[test]
    fn natural_ratio_translates_padded_border_box_percentage_and_bound() {
        let mut style = Computed {
            height: Some(Len::Pct(1.0)),
            max_height: Some(Len::Px(100.0)),
            cb_height_def: true,
            border_box: Some(true),
            ..Computed::default()
        };
        style.padding.left = Some(Len::Px(10.0));
        style.padding.right = Some(Len::Px(10.0));
        style.padding.top = Some(Len::Px(10.0));
        style.padding.bottom = Some(Len::Px(10.0));
        let mut holder = crate::apply::apply(div(), &style);
        assert!(!apply(&mut holder, &style, Some(113.0 / 120.0), false));
        assert_eq!(holder.style().content_box, Some(true));
        assert_eq!(
            holder.style().size.height,
            Some(DefiniteLength::Calc(-20.0, 1.0).into())
        );
        assert_eq!(holder.style().max_size.height, Some(px(80.0).into()));
        assert_eq!(holder.style().aspect_ratio, Some(113.0 / 120.0));
    }

    #[test]
    fn natural_content_box_removes_preflattened_padding_from_bounds() {
        let mut style = Computed {
            height: Some(Len::Pct(1.0)),
            max_height: Some(Len::Px(100.0)),
            cb_height_def: true,
            ..Computed::default()
        };
        style.padding.top = Some(Len::Px(7.0));
        style.padding.bottom = Some(Len::Px(7.0));
        let mut holder = crate::apply::apply(div(), &style);
        assert_eq!(holder.style().max_size.height, Some(px(114.0).into()));
        assert!(!apply(&mut holder, &style, Some(113.0 / 120.0), false));
        assert_eq!(holder.style().max_size.height, Some(px(100.0).into()));
        assert_eq!(holder.style().content_box, Some(true));
    }
    #[test]
    fn unresolved_percentage_block_bounds_remain_unresolved() {
        let style = Computed {
            width: Some(Len::Pct(1.0)),
            max_height: Some(Len::Pct(0.5)),
            ..Computed::default()
        };
        let mut holder = crate::apply::apply(div(), &style);
        assert_eq!(holder.style().max_size.height, None);
        assert!(!apply(&mut holder, &style, Some(113.0 / 120.0), false));
        assert_eq!(holder.style().max_size.height, None);
        assert_eq!(holder.style().size.height, None);
        assert_eq!(holder.style().aspect_ratio, Some(113.0 / 120.0));
    }
}
