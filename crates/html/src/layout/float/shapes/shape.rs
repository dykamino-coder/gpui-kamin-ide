//! Форма выреза флоата (shape-outside): коробки, отступ формы, многоугольник.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::tolerance::COORD_EPS;

#[allow(clippy::too_many_arguments)]
pub(crate) fn float_shape_of(
    vert_lr: bool,
    line_left_bottom: bool,
    f: &Element,
    side: i32,
    ml: f32,
    mt: f32,
    bl: f32,
    bt: f32,
    pl: f32,
    pt: f32,
    cw: f32,
    chh: f32,
    mw: f32,
    mh: f32,
    raw: String,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    off: f32,
    sm: f32,
    vert_rl: bool,
    vcx: ValueCx<'_>,
) -> super::super::shapes::FloatShape {
    if !vert_rl && let Some(at) = raw.find("circle(").or_else(|| raw.find("ellipse(")) {
        let inner = &raw[at..];
        let inner = match inner.find(')') {
            Some(end) => &inner[..=end],
            None => inner,
        };
        match crate::style::values::basic_shape::shape_params(inner, bw, bh, 1.0, vcx) {
            Some((cx, cy, rx, ry)) => {
                // Координаты — от опорной коробки; переводим к margin-box.
                let (cx, cy) = (cx + bx, cy + by);
                let cx = if side < 0 { cx } else { mw - cx };
                let (rx, ry) = (rx + sm, ry + sm);
                // css-shapes-1 §3.1: «When a shape is used to define a
                // float area, the shape is clipped to the float's margin
                // box». Аналитический эллипс клипа не знает: круг
                // `at left top` второго флоата лез на радиус ВЫШЕ своего
                // флоата и выталкивал коробки под него (`circle-032`:
                // длинная коробка на y=180 вместо 60), `circle(100%)`
                // отдавал экстент шире margin-box (`circle-041`: 164 при
                // 120). Вылезающий эллипс идёт профилем по точке высоты:
                // тот же срез `ellipse_cut`, по высоте только [0, mh), по
                // оси зажат [0, mw] — как растровый путь
                // (`background::shape_profile`). Лежащий внутри — прежней
                // аналитикой, ни на сотую не меняется.
                let spills =
                    cy - ry < -COORD_EPS || cy + ry > mh + COORD_EPS || cx + rx > mw + COORD_EPS;
                if spills {
                    let rows = mh.ceil().max(1.0) as usize;
                    let ext: Vec<f32> = (0..rows)
                        .map(|r| {
                            let y0 = r as f32;
                            let v = crate::layout::float::shapes::ellipse_cut(
                                cy,
                                rx,
                                ry,
                                cx,
                                y0,
                                (y0 + 1.0).min(mh),
                            )
                            .clamp(0.0, mw);
                            if v > 0.0 { off + v } else { 0.0 }
                        })
                        .collect();
                    crate::layout::float::shapes::FloatShape::Profile {
                        top: 0.0,
                        ext: std::sync::Arc::new(ext),
                    }
                } else {
                    crate::layout::float::shapes::FloatShape::Ellipse {
                        top: 0.0,
                        cx: cx + off,
                        cy,
                        rx,
                        ry,
                    }
                }
            }
            None => crate::layout::float::shapes::FloatShape::Band {
                top: 0.0,
                h: mh,
                w: off + mw + sm,
            },
        }
    } else {
        // Geometric rounded boxes stay continuous; raster shapes retain dilation.
        let radius_of = |c: &Option<crate::style::values::value::Len>| match c {
            Some(crate::style::values::value::Len::Px(v)) => (*v, *v),
            Some(crate::style::values::value::Len::Pct(k)) => (k * bw, k * bh),
            _ => (0.0, 0.0),
        };
        let sb = crate::style::values::basic_shape::ShapeBox {
            mw,
            mh,
            rx: bx,
            ry: by,
            rw: bw,
            rh: bh,
            cx: ml + bl + pl,
            cy: mt + bt + pt,
            cw,
            ch: chh,
            // Elliptical corners (`60px 40px`, css-backgrounds-3 §5.1) keep
            // both radii (`shape-outside-border-box-border-radius-007`).
            // Each axis resolves a percentage against its own box side.
            radius: {
                let ell = f.style.cascaded().radius_ell.unwrap_or([None; 4]);
                let axis = |l: crate::style::values::value::Len, base: f32| match l {
                    crate::style::values::value::Len::Px(v) => v,
                    crate::style::values::value::Len::Pct(k) => k * base,
                    _ => 0.0,
                };
                let pair = |i: usize, c: &Option<crate::style::values::value::Len>| match ell[i] {
                    Some((x, y)) => (axis(x, bw), axis(y, bh)),
                    None => radius_of(c),
                };
                [
                    pair(0, &f.style.cascaded().radius.tl),
                    pair(1, &f.style.cascaded().radius.tr),
                    pair(2, &f.style.cascaded().radius.br),
                    pair(3, &f.style.cascaded().radius.bl),
                ]
            },
            threshold: f.style.cascaded().shape_threshold.unwrap_or(0.0),
        };
        if let Some(shape) = (!vert_rl && sm <= 0.0)
            .then(|| super::rounded_float::rounded_float(&raw, &sb, side, vcx))
            .flatten()
        {
            crate::layout::float::shapes::FloatShape::RoundedBox {
                top: 0.0,
                off,
                shape: std::sync::Arc::new(shape),
            }
        } else {
            let profile = if vert_rl {
                // Профиль адресуется от блок-старта: у `vertical-rl` это
                // правый край (так его и строит `shape_profile_block`), у
                // `vertical-lr` — левый, то есть тот же профиль задом наперёд.
                let pside = if line_left_bottom { -side } else { side };
                super::profile::shape_profile_block(
                    (&raw, f.style.cascaded().shape_image.as_ref()),
                    &sb,
                    sm.max(0.0),
                    pside,
                    vcx,
                )
                .map(|mut p| {
                    if vert_lr {
                        p.reverse();
                    }
                    p
                })
            } else {
                super::profile::shape_profile(
                    (&raw, f.style.cascaded().shape_image.as_ref()),
                    &sb,
                    sm.max(0.0),
                    side,
                    vcx,
                )
            };
            match profile {
                Some(ext) => crate::layout::float::shapes::FloatShape::Profile {
                    top: 0.0,
                    ext: std::sync::Arc::new(
                        ext.into_iter()
                            .map(|v| if v > 0.0 && !vert_rl { off + v } else { v })
                            .collect(),
                    ),
                },
                None if vert_rl => {
                    // Непонятная запись в вертикали: занята вся блок-ось
                    // margin-box на всю его инлайн-ось.
                    crate::layout::float::shapes::FloatShape::Band {
                        top: 0.0,
                        h: mw,
                        w: mh,
                    }
                }
                None => {
                    // Непонятная запись: прямоугольник опорной коробки со
                    // стороны текста.
                    let w_cut = if side < 0 { bx + bw } else { mw - bx };
                    crate::layout::float::shapes::FloatShape::Band {
                        top: by,
                        h: bh,
                        w: off + w_cut + sm,
                    }
                }
            }
        }
    }
}
