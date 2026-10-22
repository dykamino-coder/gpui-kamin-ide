//! `object-view-box` (css-images-4 §object-view-box): видимая часть природного объекта
//! становится его новым природным размером. Здесь — вырез в точках природного растра и
//! размер коробки по нему; рисует вырез, растянутый на коробку, покраска.

use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_strict};

/// Вырез и коробка картинки с `object-view-box`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewBoxSize {
    /// Вырез `(x, y, w, h)` в точках природного растра.
    pub(crate) view: (f32, f32, f32, f32),
    /// Размер коробки.
    pub(crate) size: (f32, f32),
}

/// Вид записи сводится к прямоугольнику в точках природного растра `natural`:
/// `inset(t r b l)`, `rect(t r b l)` (правый и нижний края от левого/верхнего края
/// объекта), `xywh(x y w h)`. Коробка: размер из обособления (`contain-intrinsic-size`)
/// или заданный, иначе из выреза — с соотношением выреза для одной заданной стороны.
/// `None` — вырез пуст.
pub(crate) fn view_box_size(
    style: &Computed,
    vb: (u8, [Len; 4]),
    natural: (f32, f32),
) -> Option<ViewBoxSize> {
    let (w0, h0) = natural;
    let at = |l: Len, base: f32| match l {
        Len::Px(v) => v,
        Len::Pct(k) => k * base,
        _ => 0.0,
    };
    let [a, b, c, d] = vb.1;
    let (vx, vy, vw, vh) = match vb.0 {
        // inset(top right bottom left)
        0 => (
            at(d, w0),
            at(a, h0),
            w0 - at(d, w0) - at(b, w0),
            h0 - at(a, h0) - at(c, h0),
        ),
        // rect(top right bottom left)
        1 => (
            at(d, w0),
            at(a, h0),
            at(b, w0) - at(d, w0),
            at(c, h0) - at(a, h0),
        ),
        // xywh(x y w h)
        _ => (at(a, w0), at(b, h0), at(c, w0), at(d, h0)),
    };
    if !(vw > 0.0 && vh > 0.0) {
        return None;
    }
    let px_of = px_strict;
    let (bw, bh) = if style.contains_width() || style.contains_height() {
        (
            px_of(style.width)
                .or(style.contain_intrinsic.0)
                .unwrap_or(0.0),
            px_of(style.height)
                .or(style.contain_intrinsic.1)
                .unwrap_or(0.0),
        )
    } else {
        match (px_of(style.width), px_of(style.height)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * vh / vw),
            (None, Some(h)) => (h * vw / vh, h),
            (None, None) => (vw, vh),
        }
    };
    Some(ViewBoxSize {
        view: (vx, vy, vw, vh),
        size: (bw, bh),
    })
}
