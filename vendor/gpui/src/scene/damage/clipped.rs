//! KaminIDE patch: равенство сплошных квадов по видимой части.
//!
//! Пиксели сплошной заливки без рамки, скругления и преобразования зависят
//! только от краёв, лежащих внутри маски. Маска отсекает по центру пикселя
//! (`SV_ClipDistance`, у CPU-растра — так же), а сглаживание края — полоса
//! `0.5 − sdf` шириной полпикселя (`antialias_threshold`): пиксель, задетый
//! маской, лежит до `ceil(M)` и покрыт целиком, если край квада не ближе
//! `ceil(M)`. Такой край ни одного пикселя под маской не меняет.
//! Поэтому фон холста шириной 801 и 800 px под маской кадра 800 px рисует
//! одно и то же, а точное сравнение значений видело в нём повреждение всего
//! кадра (effects-480 при смене ширины: 130 из 130 клеток, полный растр).
//! Так же, по видимой части, сравнивает слои Chromium (`DamageTracker`
//! объединяет видимые прямоугольники, а не исходные границы слоя).

use crate::{BackgroundTag, Quad, ScaledPixels, TransformationMatrix};


/// Квады `a` и `b` (без `order`) рисуют под общей маской одни пиксели, хотя
/// их границы различаются.
pub(super) fn same_visible(a: &Quad, b: &Quad) -> bool {
    if !plain(a) || !plain(b) {
        return false;
    }
    let mut x = *a;
    x.order = b.order;
    x.bounds = b.bounds;
    if x != *b {
        return false;
    }
    let m = a.content_mask.bounds;
    let edges = |q: &Quad| {
        let o = q.bounds.origin;
        let s = q.bounds.size;
        [o.x.0, o.y.0, o.x.0 + s.width.0, o.y.0 + s.height.0]
    };
    let (ea, eb) = (edges(a), edges(b));
    let mask = [
        m.origin.x.0,
        m.origin.y.0,
        m.origin.x.0 + m.size.width.0,
        m.origin.y.0 + m.size.height.0,
    ];
    (0..4).all(|k| {
        let beyond = |v: f32| {
            if k < 2 {
                v <= mask[k].floor()
            } else {
                v >= mask[k].ceil()
            }
        };
        ea[k].to_bits() == eb[k].to_bits() || (beyond(ea[k]) && beyond(eb[k]))
    })
}

/// Сплошная заливка без рамки, скругления и преобразования.
pub(super) fn plain(q: &Quad) -> bool {
    let zero = |v: ScaledPixels| v.0 == 0.0;
    let w = q.border_widths;
    let r = q.corner_radii;
    q.background.tag == BackgroundTag::Solid
        && [w.top, w.right, w.bottom, w.left].into_iter().all(zero)
        && [r.top_left, r.top_right, r.bottom_right, r.bottom_left]
            .into_iter()
            .all(zero)
        && q.transformation == TransformationMatrix::unit()
        && [q.bounds.origin.x.0, q.bounds.origin.y.0, q.bounds.size.width.0, q.bounds.size.height.0]
            .into_iter()
            .all(f32::is_finite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Background, ContentMask, Corners, bounds, point, red, size};

    fn quad(x0: f32, x1: f32) -> Quad {
        Quad {
            bounds: bounds(
                point(ScaledPixels(x0), ScaledPixels(0.0)),
                size(ScaledPixels(x1 - x0), ScaledPixels(600.0)),
            ),
            content_mask: ContentMask {
                bounds: bounds(
                    point(ScaledPixels(0.0), ScaledPixels(0.0)),
                    size(ScaledPixels(800.0), ScaledPixels(600.0)),
                ),
            },
            background: Background::from(red()),
            ..Default::default()
        }
    }

    #[test]
    fn edges_on_or_past_the_mask_do_not_matter() {
        // Фон холста при ширине 800 и 801 под маской кадра 800.
        assert!(same_visible(&quad(0.0, 800.0), &quad(0.0, 801.0)));
        assert!(same_visible(&quad(0.0, 800.5), &quad(0.0, 1200.0)));
        assert!(same_visible(&quad(-3.0, 801.0), &quad(0.0, 900.0)));
    }

    #[test]
    fn edges_inside_the_mask_still_differ() {
        // Край внутри маски задевает её пиксели сглаживанием.
        assert!(!same_visible(&quad(0.0, 799.6), &quad(0.0, 1200.0)));
        assert!(!same_visible(&quad(0.4, 900.0), &quad(0.0, 900.0)));
        assert!(!same_visible(&quad(10.0, 900.0), &quad(11.0, 900.0)));
    }

    #[test]
    fn only_plain_solid_quads_compare_by_the_visible_part() {
        let (mut a, mut b) = (quad(0.0, 801.0), quad(0.0, 900.0));
        a.corner_radii = Corners::all(ScaledPixels(4.0));
        b.corner_radii = Corners::all(ScaledPixels(4.0));
        assert!(!same_visible(&a, &b));
        let (mut a, mut b) = (quad(0.0, 801.0), quad(0.0, 900.0));
        a.background = Background::from(crate::blue());
        assert!(!same_visible(&a, &b));
        b.background = a.background;
        assert!(same_visible(&a, &b));
    }
}
