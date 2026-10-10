//! SVG-тень контура рамки, вынесенная из построения пути.

use crate::paint::background::shape_path::border_geometry::miter_limit;
use crate::paint::background::*;

/// Разметка теней `box-shadow` у коробки с `border-shape` (css-borders-4
/// §border-shape-shadow-interaction: «cast as if the shape defined by the
/// outer path were opaque … expanded or contracted by the spread distance,
/// blurred by the blur radius, and then clipped by the border-shape»).
/// Холст, border-box, `outer`/`inner` — как у `border_shape_ring_svg`.
///
/// Наружная тень (Blink `BoxPainterBase::PaintNormalBoxShadow`,
/// `box_painter_base.cc:275-372` ветка `HasBorderShape`): контур внешней
/// фигуры, раздутый на `spread` плюс половина обводки у одной фигуры
/// (`BorderShapePainter::OuterPathWithOffset`, `border_shape_painter.cc:135-195`),
/// сдвинутый на смещение и размытый гауссом σ = blur/2 (`BlurAsSigma`), минус
/// область «фигура ∪ обводка» (`OuterPath`, `:79-103`, клип `kDifference`).
/// Внутренняя (`PaintInsetBoxShadowForBorderShape`, `:410-495`): всё вне
/// внутреннего контура (`InnerPath` = фигура минус обводка, `:105-133`),
/// дыра сжата на `spread` (отрицательный — расширена), сдвинута и размыта;
/// видна только внутри `InnerPath`.
///
/// Покрытие каждой тени собирается в `<mask>` светимостью (белый контур с
/// обводкой — раздутие, чёрный — вырез), а краска кладётся одним
/// прямоугольником сквозь маску: полупрозрачный цвет не красится дважды там,
/// где заливка и обводка перекрываются (border-shape-shadow-semitransparent;
/// Blink для того же берёт объединение путей). Порядок — от последней тени к
/// первой (§box-shadow: «the first shadow is on top»).
pub fn border_shape_shadow_svg(
    outer: (&str, [f32; 4]),
    inner: Option<(&str, [f32; 4])>,
    stroke: f32,
    shadows: &[(
        crate::style::computed::Shadow,
        crate::style::values::value::Color,
    )],
    inset: bool,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let place = |(raw, [t, r, b, l]): (&str, [f32; 4])| -> Option<(String, String, &'static str)> {
        let (d, rule) = border_shape_path(raw, (bw + l + r).max(0.0), (bh + t + b).max(0.0))?;
        Some((format!("translate({} {})", dx - l, dy - t), d, rule))
    };
    let (tr_o, d_o, rule_o) = place(outer)?;
    if d_o.is_empty() {
        return None;
    }
    // Одна фигура: обводка по центру контура — «фигура ∪ обводка» наружу и
    // «фигура − обводка» внутрь на половину толщины; две — контуры как есть.
    let half = if inner.is_some() { 0.0 } else { stroke / 2.0 };
    let ml_o = miter_limit(outer.0);
    // Внутренний контур: своя фигура у двух, внешняя у одной.
    let (tr_i, d_i, rule_i, ml_i) = match inner {
        Some(inner) => {
            let (tr, d, rule) = place(inner)?;
            (tr, d, rule, miter_limit(inner.0))
        }
        None => (tr_o.clone(), d_o.clone(), rule_o, ml_o),
    };
    let rgb = |c: crate::style::values::value::Color| {
        format!(
            "rgb({},{},{})",
            (c.r * 255.0).round(),
            (c.g * 255.0).round(),
            (c.b * 255.0).round()
        )
    };
    let mut defs = String::new();
    let mut body = String::new();
    for (i, (sh, colour)) in shadows.iter().enumerate().rev() {
        if colour.a <= 0.0 {
            continue;
        }
        let sigma = sh.blur.max(0.0) / 2.0;
        let (f_open, f_close) = if sigma > 0.0 {
            defs.push_str(&format!(
                r##"<filter id="bsf{i}" filterUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><feGaussianBlur stdDeviation="{sigma}"/></filter>"##
            ));
            (
                format!(r##"<g filter="url(#bsf{i})">"##),
                "</g>".to_string(),
            )
        } else {
            (String::new(), String::new())
        };
        let (ox, oy) = (sh.x, sh.y);
        if !inset {
            // Раздутие на `spread + half`: белая обводка вдвое шире сверх
            // заливки; сжатие (минус) — чёрная обводка поверх заливки.
            let total = sh.spread + half;
            let caster = if total > 0.0 {
                format!(
                    r##"<path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml_o}"/>"##,
                    total * 2.0
                )
            } else if total < 0.0 {
                format!(
                    r##"<path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}"/><path d="{d_o}" fill="none" stroke="#000000" stroke-width="{}" stroke-miterlimit="{ml_o}"/>"##,
                    -total * 2.0
                )
            } else {
                format!(r##"<path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}"/>"##)
            };
            // Вырез «фигура ∪ обводка» — тень не видна под самой рамкой.
            let cut_stroke = if half > 0.0 {
                format!(r##" stroke="#000000" stroke-width="{stroke}" stroke-miterlimit="{ml_o}""##)
            } else {
                String::new()
            };
            defs.push_str(&format!(
                r##"<mask id="bsm{i}" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}">{f_open}<g transform="translate({ox} {oy})"><g transform="{tr_o}">{caster}</g></g>{f_close}<g transform="{tr_o}"><path d="{d_o}" fill="#000000" fill-rule="{rule_o}"{cut_stroke}/></g></mask>"##
            ));
            body.push_str(&format!(
                r##"<rect x="0" y="0" width="{cw}" height="{ch}" fill="{}" fill-opacity="{}" mask="url(#bsm{i})"/>"##,
                rgb(*colour),
                colour.a
            ));
        } else {
            // Дыра = внутренний контур, сжатый на `spread + half`: белая
            // обводка возвращает полосу бросающему; отрицательный разлёт
            // расширяет дыру чёрной обводкой.
            let total = sh.spread + half;
            let hole = if total > 0.0 {
                format!(
                    r##"<path d="{d_i}" fill="#000000" fill-rule="{rule_i}"/><path d="{d_i}" fill="none" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml_i}"/>"##,
                    total * 2.0
                )
            } else if total < 0.0 {
                format!(
                    r##"<path d="{d_i}" fill="#000000" fill-rule="{rule_i}" stroke="#000000" stroke-width="{}" stroke-miterlimit="{ml_i}"/>"##,
                    -total * 2.0
                )
            } else {
                format!(r##"<path d="{d_i}" fill="#000000" fill-rule="{rule_i}"/>"##)
            };
            // Видимость — только внутри `InnerPath` (фигура минус обводка).
            let clip_stroke = if half > 0.0 {
                format!(r##" stroke="#000000" stroke-width="{stroke}" stroke-miterlimit="{ml_i}""##)
            } else {
                String::new()
            };
            defs.push_str(&format!(
                r##"<mask id="bsc{i}" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><g transform="{tr_i}"><path d="{d_i}" fill="#ffffff" fill-rule="{rule_i}"{clip_stroke}/></g></mask><mask id="bsm{i}" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}">{f_open}<g transform="translate({ox} {oy})"><rect x="-10000" y="-10000" width="20000" height="20000" fill="#ffffff"/><g transform="{tr_i}">{hole}</g></g>{f_close}</mask>"##
            ));
            body.push_str(&format!(
                r##"<g mask="url(#bsc{i})"><rect x="0" y="0" width="{cw}" height="{ch}" fill="{}" fill-opacity="{}" mask="url(#bsm{i})"/></g>"##,
                rgb(*colour),
                colour.a
            ));
        }
    }
    if body.is_empty() {
        return None;
    }
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><defs>{defs}</defs>{body}</svg>"##
    ))
}
