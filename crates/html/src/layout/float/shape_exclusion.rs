//! Исключение линий `shape-outside` для измеренного float (css-shapes-1 §2, §3):
//! reference box фигуры берётся из рассчитанного border box float, его собственных
//! границ и padding и полей, разрешённых хостом.

use crate::dom::Element;
use crate::layout::float::shapes::{FloatShape, float_shape_of};
use crate::style::values::value::Len;

/// The exclusion of float `f` on side `side` (-1 line-left, 1 line-right)
/// whose border box measured `bw` × `bh` in the host's logical axes (inline,
/// block), with logical margins `[block-start, inline-end, block-end,
/// inline-start]`, in a containing block `cbw` wide in the inline axis; `em`
/// is the float's font size. `wm` maps the host's line-relative axes to
/// physical edges, including bottom line-left in sideways-lr. Measured from the
/// float's own margin edge and margin-box block-start, in logical axes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn measured_shape(
    f: &Element,
    side: i8,
    (bw, bh): (f32, f32),
    [mt, mr, mb, ml]: [f32; 4],
    cbw: f32,
    em: f32,
    wm: crate::style::computed::writing_mode::WritingMode,
    vcx: crate::document::state::ValueCx<'_>,
) -> FloatShape {
    // Physical sizes and margins (css-writing-modes-4 §6.4): in vertical
    // modes the inline axis is the height and block-start is the right
    // (`vertical-rl`) or the left (`vertical-lr`) edge.
    let vertical = wm.vertical().then_some(wm.block_rl());
    let ((bw, bh), [mt, mr, mb, ml]) = match vertical {
        None => ((bw, bh), [mt, mr, mb, ml]),
        Some(true) => ((bh, bw), [ml, mt, mr, mb]),
        Some(false) if wm.sideways_lr() => ((bh, bw), [mr, mb, ml, mt]),
        Some(false) => ((bh, bw), [ml, mb, mr, mt]),
    };
    // Paddings resolve against the containing block width (CSS 2.1 §8.4),
    // like the margins the host already resolved.
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        Some(Len::Em(k)) => k * em,
        Some(Len::Pct(k)) => k * cbw,
        _ => 0.0,
    };
    let b = f.style.used().borders();
    let (bl, br, bt, bb) = (px(&b.left), px(&b.right), px(&b.top), px(&b.bottom));
    let p = &f.style.used().padding;
    let (pl, pr, pt, pb) = (px(&p.left), px(&p.right), px(&p.top), px(&p.bottom));
    let cw = (bw - bl - br - pl - pr).max(0.0);
    let ch = (bh - bt - bb - pt - pb).max(0.0);
    let (mw, mh) = (ml + bw + mr, mt + bh + mb);
    let raw = f.style.used().shape_outside.clone().unwrap_or_default();
    // The reference box (css-shapes-1 §3.3), margin-box by default, in
    // margin-box coordinates.
    let (bx, by, rw, rh) = if raw.contains("border-box") {
        (ml, mt, bw, bh)
    } else if raw.contains("padding-box") {
        (ml + bl, mt + bt, bw - bl - br, bh - bt - bb)
    } else if raw.contains("content-box") {
        (ml + bl + pl, mt + bt + pt, cw, ch)
    } else {
        (0.0, 0.0, mw, mh)
    };
    // `shape-margin` percentages resolve against the containing block's
    // inline size (css-shapes-1 §shape-margin-property).
    let vert_lr = vertical == Some(false);
    let sm = match f.style.used().shape_margin {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => k * cbw,
        _ => 0.0,
    };
    float_shape_of(
        vert_lr,
        wm.sideways_lr(),
        f,
        i32::from(side),
        ml,
        mt,
        bl,
        bt,
        pl,
        pt,
        cw,
        ch,
        mw,
        mh,
        raw,
        bx,
        by,
        rw,
        rh,
        0.0,
        sm,
        vertical.is_some(),
        vcx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Computed, writing_mode::WritingMode};

    #[test]
    fn sideways_shape_uses_bottom_line_edge_and_asymmetric_margins() {
        let doc = crate::document::state::DocState::new(Default::default());
        let float = Element::synthetic(
            "div",
            Computed {
                shape_outside: Some("inset(2px 0 8px 0) border-box".into()),
                ..Default::default()
            },
            Default::default(),
            vec![],
        );
        for rtl in [false, true] {
            for (side, expected) in [(-1, 39.), (1, 27.)] {
                let shape = measured_shape(
                    &float,
                    side,
                    (30., 20.),
                    [3., 5., 7., 11.],
                    100.,
                    16.,
                    WritingMode::new(true, false, true, rtl),
                    doc.values(),
                );
                assert_eq!(shape.cut(0., 2.), 0.);
                assert!((shape.cut(4., 5.) - expected).abs() < crate::tolerance::COORD_EPS);
                assert_eq!(shape.cut(24., 25.), 0.);
            }
        }
    }
}
