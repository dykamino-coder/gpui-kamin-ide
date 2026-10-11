//! Форма выреза флоата (shape-outside): коробки, отступ формы, многоугольник.

use crate::dom::Element;

#[allow(clippy::too_many_arguments)]
pub(super) fn float_shape_of(
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
) -> super::super::shapes::FloatShape {
    if !vert_rl && let Some(at) = raw.find("circle(").or_else(|| raw.find("ellipse(")) {
        let inner = &raw[at..];
        let inner = match inner.find(')') {
            Some(end) => &inner[..=end],
            None => inner,
        };
        match crate::paint::background::shape_params(inner, bw, bh, 1.0) {
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
                const EPS: f32 = 0.01;
                let spills = cy - ry < -EPS || cy + ry > mh + EPS || cx + rx > mw + EPS;
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
        let sb = crate::paint::background::ShapeBox {
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
                let ell = f.style.radius_ell.unwrap_or([None; 4]);
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
                    pair(0, &f.style.radius.tl),
                    pair(1, &f.style.radius.tr),
                    pair(2, &f.style.radius.br),
                    pair(3, &f.style.radius.bl),
                ]
            },
            threshold: f.style.shape_threshold.unwrap_or(0.0),
        };
        if let Some(shape) = (!vert_rl && sm <= 0.0)
            .then(|| crate::paint::background::rounded_float(&raw, &sb, side))
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
                crate::paint::background::shape_profile_block(&raw, &sb, sm.max(0.0), pside).map(
                    |mut p| {
                        if vert_lr {
                            p.reverse();
                        }
                        p
                    },
                )
            } else {
                crate::paint::background::shape_profile(&raw, &sb, sm.max(0.0), side)
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
