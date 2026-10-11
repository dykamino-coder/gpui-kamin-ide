//! SVG-маски контура и кольца рамки.

use crate::paint::background::shape_path::border_geometry::miter_limit;
use crate::paint::background::*;

/// Разметка маски буфера группы под `border-shape`: холст cw×ch — область
/// композита, border-box в нём начинается в (dx, dy) и имеет размер bw×bh.
/// Запись `spec`: `t r b l` края опорной коробки от border-box (наружу
/// положительные), толщина обводки, `t r b l` выноса области, `:` и текст
/// фигуры. Одна фигура — контур ПЛЮС его обводка толщиной рамки (Blink
/// `BorderShapePainter::OuterPath` = shape ∪ stroke: фон и содержимое видны
/// под всем кольцом); две — внешняя фигура как есть (обводка 0).
pub fn border_shape_mask_svg(
    spec: &str,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let (head, raw) = spec.split_once(':')?;
    let v: Vec<f32> = head
        .split_whitespace()
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    if v.len() != 9 {
        return None;
    }
    let (ot, or_, ob, ol, stroke) = (v[0], v[1], v[2], v[3], v[4]);
    let (rw, rh) = ((bw + ol + or_).max(0.0), (bh + ot + ob).max(0.0));
    let (d, rule) = border_shape_path(raw, rw, rh)?;
    let path = if d.is_empty() {
        String::new()
    } else if stroke < 0.0 {
        // Отрицательная обводка — ВНУТРЕННИЙ контур одной фигуры: «фигура
        // минус обводка» (Blink `BorderShapePainter::InnerPath`,
        // `border_shape_painter.cc:105-133`) — чёрная обводка поверх белой
        // заливки съедает полосу внутрь на половину толщины. Так режется
        // переполнение (css-borders-4 §border-shape-overflow-interaction).
        format!(
            r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}"/><path d="{d}" fill="none" stroke="#000000" stroke-width="{}" stroke-miterlimit="{}"/>"##,
            -stroke,
            miter_limit(raw)
        )
    } else {
        let stroke_attr = if stroke > 0.0 {
            format!(
                r##" stroke="#ffffff" stroke-width="{stroke}" stroke-miterlimit="{}""##,
                miter_limit(raw)
            )
        } else {
            String::new()
        };
        format!(r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}"{stroke_attr}/>"##)
    };
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><g transform="translate({} {})">{path}</g></svg>"##,
        dx - ol,
        dy - ot
    ))
}

/// Разметка кольца рамки `border-shape` цветом `colour` (холст и border-box —
/// как у `border_shape_mask_svg`; `outer`/`inner` — текст фигуры и края её
/// опорной коробки от border-box). Одна фигура — SVG-обводка толщиной
/// `stroke` по центру контура (half-border-box: поровну внутрь и наружу);
/// две — «внешняя минус внутренняя» через `<mask>` (не evenodd: внутренняя
/// может выходить за внешнюю, Blink берёт разность путей). `None` — рисовать
/// нечего.
pub fn border_shape_ring_svg(
    outer: (&str, [f32; 4]),
    inner: Option<(&str, [f32; 4])>,
    stroke: f32,
    colour: crate::style::values::value::Color,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let rgb = format!(
        "rgb({},{},{})",
        (colour.r * 255.0).round(),
        (colour.g * 255.0).round(),
        (colour.b * 255.0).round()
    );
    let place = |(raw, [t, r, b, l]): (&str, [f32; 4])| -> Option<(String, String, &'static str)> {
        let (d, rule) = border_shape_path(raw, (bw + l + r).max(0.0), (bh + t + b).max(0.0))?;
        Some((format!("translate({} {})", dx - l, dy - t), d, rule))
    };
    let (tr_o, d_o, rule_o) = place(outer)?;
    if d_o.is_empty() {
        return None;
    }
    let body = match inner {
        None => {
            if stroke <= 0.0 {
                return None;
            }
            format!(
                r##"<g transform="{tr_o}"><path d="{d_o}" fill="none" fill-rule="{rule_o}" stroke="{rgb}" stroke-opacity="{}" stroke-width="{stroke}" stroke-miterlimit="{}"/></g>"##,
                colour.a,
                miter_limit(outer.0)
            )
        }
        Some(inner) => {
            let (tr_i, d_i, rule_i) = place(inner)?;
            let hole = if d_i.is_empty() {
                String::new()
            } else {
                format!(
                    r##"<g transform="{tr_i}"><path d="{d_i}" fill="#000000" fill-rule="{rule_i}"/></g>"##
                )
            };
            format!(
                r##"<mask id="ring" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><g transform="{tr_o}"><path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}"/></g>{hole}</mask><rect width="{cw}" height="{ch}" fill="{rgb}" fill-opacity="{}" mask="url(#ring)"/>"##,
                colour.a
            )
        }
    };
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}">{body}</svg>"##
    ))
}
