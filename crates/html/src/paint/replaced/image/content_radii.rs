//! Внутренние радиусы края содержимого у замещаемой картинки: радиусы `border-radius`,
//! уменьшенные на рамку и отбивку (круговое приближение); нули при угловой форме
//! `corner-shape`, чтобы обрезка картинки не стирала контур коробки.

use crate::style::values::value::Len;

/// Радиусы углов края содержимого (tl, tr, br, bl) в точках; круговое
/// приближение — меньшая из двух осей угла.
pub(super) fn content_radii(s: &crate::style::computed::Computed) -> [f32; 4] {
    // A circular image clip would erase the corner-shaped CSS box contour.
    if s.corner_shaped() {
        return [0.0; 4];
    }
    let v = |l: Option<Len>| match l {
        Some(Len::Px(x)) => x.max(0.0),
        _ => 0.0,
    };
    let b = s.borders();
    let (top, right, bottom, left) = (
        v(b.top) + v(s.padding.top),
        v(b.right) + v(s.padding.right),
        v(b.bottom) + v(s.padding.bottom),
        v(b.left) + v(s.padding.left),
    );
    let inner = |r: f32, h: f32, w: f32| (r - h.max(w)).max(0.0);
    [
        inner(v(s.radius.tl), left, top),
        inner(v(s.radius.tr), right, top),
        inner(v(s.radius.br), right, bottom),
        inner(v(s.radius.bl), left, bottom),
    ]
}
