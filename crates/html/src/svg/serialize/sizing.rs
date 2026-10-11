//! Сериализация SVG с заданным размером области просмотра.

use super::{escape_attr, escape_text, write_element};
use crate::dom::{Element, Node};
use crate::svg::serialize::filters::subtree_has;
use crate::svg::serialize::filters::untaint_filters;
use crate::svg::size::{size_of, view_box_ratio};
use crate::svg::{IN_CLIP, VIEW_BOX};

/// Разметка с корневой канвой ЗАДАННОГО размера: собственные width/height
/// корня заменяются, иначе растеризатор вписал бы рисунок по ним.
pub(crate) fn serialize_sized(e: &Element, w: f32, h: f32, nx: f32, ny: f32) -> String {
    let cleaned;
    let e = if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        cleaned = copy;
        &cleaned
    } else {
        e
    };
    let mut out = String::new();
    // Канва с запасом влево/вверх (`nx`, `ny` — переполнение в минус).
    let (cw, ch) = (w + nx, h + ny);
    out.push_str(&format!("<svg width=\"{cw}\" height=\"{ch}\""));
    // Пространство имён обязательно: разметка идёт растеризатору отдельным
    // документом. В XHTML рисунок часто пишут с ПРЕФИКСОМ (`<svg:svg
    // xmlns:svg=…>`), собственного `xmlns` у корня нет, а имена мы храним
    // без префикса — без этой строки usvg не разбирал документ вовсе, и
    // рисунок пропадал целиком (вся семья `*-replaced-*` в CSS2).
    if !e.attrs.iter().any(|(k, _)| k == "xmlns") {
        out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    // Под зумом без `viewBox` пользовательские единицы обязаны вырасти вместе
    // с канвой (длины внутри рисунка — тоже used-значения, css-viewport-1
    // §zoom): `viewBox` в незумленных единицах растягивает содержимое на
    // зумленную канву (`zoom/svg-path`, `zoom/svg`). С `viewBox` масштаб уже
    // задаёт он сам.
    let z = e.style.zoom_eff.unwrap_or(1.0);
    let shifted = nx > 0.0 || ny > 0.0;
    if ((z - 1.0).abs() > f32::EPSILON || shifted)
        && !e
            .attrs
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("viewbox"))
    {
        out.push_str(&format!(
            " viewBox=\"{} {} {} {}\"",
            -nx / z,
            -ny / z,
            cw / z,
            ch / z
        ));
    }
    // Канва шире коробки (`overflow: visible`, см. `element`): при `viewBox`
    // пользовательские единицы обязаны остаться прежними, иначе рисунок
    // растянулся бы на выросшую канву (`border-shape-shadow-ref`: рамка
    // 220 → 230). `viewBox` расширяется в той же пропорции, что и канва, а
    // при запасе влево/вверх — сдвигается на него.
    let (w0, h0) = size_of(e);
    let grown = w > w0 + 0.5 || h > h0 + 0.5 || shifted;
    for (k, v) in &e.attrs {
        if k == "width" || k == "height" {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        if grown && k.eq_ignore_ascii_case("viewbox") {
            let p: Vec<f32> = v
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            if p.len() == 4 && w0 > 0.0 && h0 > 0.0 {
                let (ux, uy) = (p[2] / w0, p[3] / h0);
                out.push_str(&format!(
                    "{} {} {} {}",
                    p[0] - nx * ux,
                    p[1] - ny * uy,
                    cw * ux,
                    ch * uy
                ));
                out.push('"');
                continue;
            }
        }
        escape_attr(v, &mut out);
        out.push('"');
    }
    out.push('>');
    // Опорная коробка `view-box` для CSS-обрезки детей (`synth_clip`): корень
    // пишется здесь, а не в `write_element`.
    VIEW_BOX.with(|v| v.set(view_box_ratio(e).unwrap_or_else(|| size_of(e))));
    IN_CLIP.with(|c| c.set(false));
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, &mut out),
            Node::Element(el) => write_element(el, &mut out),
        }
    }
    out.push_str("</svg>");
    out
}
