//! A box's corners as quarter ellipses for the quad (`Computed::radius_elliptic`).

use crate::paint::apply::radius_px;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::values::ValueCx;
use gpui::px;

/// The box's corners as quarter ellipses (`Computed::radius_elliptic`): per
/// corner its horizontal and vertical radius, a percentage of the box's own
/// width and height (css-backgrounds-3 §5.1); one radius gives both. The
/// quad reduces them by the factor of §5.5 against the painted box
/// (`gpui::EllipseRadii::to_pixels`).
pub(crate) fn ellipse_radii(c: &Computed, vcx: ValueCx<'_>) -> gpui::EllipseRadii {
    let length = |l: Option<Len>| match l {
        Some(Len::Pct(p)) => gpui::DefiniteLength::Fraction(p),
        l => gpui::DefiniteLength::Absolute(gpui::AbsoluteLength::Pixels(px(
            radius_px(c, l, vcx).unwrap_or(0.0)
        ))),
    };
    let r = &c.reset_border.radius;
    let round = [r.tl, r.tr, r.br, r.bl];
    let ell = c.reset_border.radius_ell.unwrap_or([None; 4]);
    let pair = |i: usize| match ell[i] {
        Some((x, y)) => (Some(x), Some(y)),
        None => (round[i], round[i]),
    };
    let pairs = [pair(0), pair(1), pair(2), pair(3)];
    gpui::EllipseRadii {
        x: pairs.map(|(x, _)| length(x)),
        y: pairs.map(|(_, y)| length(y)),
    }
}
