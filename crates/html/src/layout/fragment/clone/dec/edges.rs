//! Усечение клонированных краёв при невозможности продвижения (css-break-3 §4.4).

use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn truncate(style: &mut Computed, dt: f32, db: f32, fh: f32, clip: f32) -> (f32, f32) {
    let budget = (fh - clip.min(1.0)).max(0.0);
    let top = dt.min(budget);
    let bottom = db.min((budget - top).max(0.0));
    if top != dt || bottom != db {
        let borders = style.borders();
        for (padding, border, used, total, kept) in [
            (
                &mut style.padding.top,
                &mut style.border_width.top,
                borders.top,
                dt,
                top,
            ),
            (
                &mut style.padding.bottom,
                &mut style.border_width.bottom,
                borders.bottom,
                db,
                bottom,
            ),
        ] {
            let border_size = match used {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            // Обрезается внешняя часть края: сначала рамка, затем padding.
            let remaining_border = (border_size - (total - kept)).max(0.0);
            *border = Some(Len::Px(remaining_border));
            *padding = Some(Len::Px((kept - remaining_border).max(0.0)));
        }
    }
    (top, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_then_start_are_cut_to_leave_content_progress() {
        let mut style = Computed::default();
        style.padding.top = Some(Len::Px(8.0));
        style.border_width.top = Some(Len::Px(2.0));
        style.border_visible[0] = Some(true);
        style.padding.bottom = Some(Len::Px(20.0));
        assert_eq!(truncate(&mut style, 10.0, 20.0, 5.0, 1.0), (4.0, 0.0));
        assert_eq!(style.padding.top, Some(Len::Px(4.0)));
        assert_eq!(style.border_width.top, Some(Len::Px(0.0)));
        assert_eq!(style.padding.bottom, Some(Len::Px(0.0)));
    }
}
