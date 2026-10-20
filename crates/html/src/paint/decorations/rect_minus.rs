//! Покраска прямоугольной области за вычетом отверстия с привязкой к сетке устройства.

use gpui::px;

/// Fill `outer` minus `hole` (both `(left, top, right, bottom)` in window
/// points) with up to four rectangles whose edges snap to the device grid —
/// the shape of a sharp, square box shadow (css-backgrounds-3 §7.1). An empty
/// or disjoint hole leaves `outer` whole.
pub(crate) fn paint_rect_minus(
    window: &mut gpui::Window,
    (ol, ot, or, ob): (f32, f32, f32, f32),
    (hl, ht, hr, hb): (f32, f32, f32, f32),
    fill: gpui::Hsla,
) {
    let sf = window.scale_factor();
    let snap = |v: f32| (v * sf).round() / sf;
    let mut paint = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let (x0, y0, x1, y1) = (snap(x0), snap(y0), snap(x1), snap(y1));
        if x1 > x0 && y1 > y0 {
            window.paint_quad(gpui::fill(
                gpui::Bounds::from_corners(
                    gpui::point(px(x0), px(y0)),
                    gpui::point(px(x1), px(y1)),
                ),
                fill,
            ));
        }
    };
    if hl >= hr || ht >= hb {
        paint(ol, ot, or, ob);
        return;
    }
    // Above and below the hole: full width.
    paint(ol, ot, or, ht.min(ob));
    paint(ol, hb.max(ot), or, ob);
    // Beside the hole, within its rows.
    let (y0, y1) = (ht.max(ot), hb.min(ob));
    paint(ol, y0, hl.min(or), y1);
    paint(hr.max(ol), y0, or, y1);
}
