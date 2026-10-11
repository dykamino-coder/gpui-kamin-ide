//! Преобразование CSS-формы в SVG path для профиля обтекания.

use super::ShapeBox;
use crate::paint::background::*;

/// Контур для SVG-растеризатора: polygon / path / shape.
pub(crate) fn svg_path_of(raw: &str, b: &ShapeBox) -> Option<(String, &'static str)> {
    let raw = raw.trim();
    // Функция может идти ПОСЛЕ слова-коробки: `padding-box polygon(...)`.
    if let Some(at) = raw.find("polygon(") {
        let inner = raw[at + 8..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 8..]);
        {
            let mut rule = "nonzero";
            let mut pts_src = inner;
            if let Some(rest) = inner.trim_start().strip_prefix("evenodd") {
                rule = "evenodd";
                pts_src = rest.trim_start().trim_start_matches(',');
            } else if let Some(rest) = inner.trim_start().strip_prefix("nonzero") {
                pts_src = rest.trim_start().trim_start_matches(',');
            }
            let len_px = |t: &str, base: f32| -> f32 {
                match crate::style::values::value::Len::parse(t) {
                    Some(crate::style::values::value::Len::Px(v)) => v,
                    Some(crate::style::values::value::Len::Pct(k)) => k * base,
                    _ => 0.0,
                }
            };
            let mut d = String::new();
            for (i, pair) in pts_src.split(',').enumerate() {
                let mut it = pair.split_whitespace();
                let x = b.rx + len_px(it.next()?, b.rw);
                let y = b.ry + len_px(it.next()?, b.rh);
                d.push_str(if i == 0 { "M" } else { "L" });
                d.push_str(&format!("{x} {y} "));
            }
            if d.is_empty() {
                return None;
            }
            d.push('Z');
            return Some((
                d,
                if rule == "evenodd" {
                    "evenodd"
                } else {
                    "nonzero"
                },
            ));
        }
    }
    if let Some(at) = raw.find("path(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        {
            // `path( [<fill-rule>,]? <string> )` — css-shapes-1 §3.1:
            // правило намотки стоит ПЕРЕД строкой контура и отделено
            // запятой. Оно не отрезалось, и слово `evenodd` вместе с
            // запятой уезжало в атрибут `d` — контур не разбирался вовсе.
            let mut rule = "nonzero";
            let mut body = inner.trim();
            if let Some(rest) = body.strip_prefix("evenodd") {
                rule = "evenodd";
                body = rest.trim_start().trim_start_matches(',').trim_start();
            } else if let Some(rest) = body.strip_prefix("nonzero") {
                body = rest.trim_start().trim_start_matches(',').trim_start();
            }
            let d = body.trim_matches('"').trim_matches('\'').to_string();
            if d.is_empty() {
                return None;
            }
            return Some((d, rule));
        }
    }
    if let Some(at) = raw.find("shape(") {
        let inner = raw[at + 6..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 6..]);
        {
            let d = shape_to_path(&inner.replace(',', ";"), b.rw, b.rh)?;
            return Some((d, "nonzero"));
        }
    }
    None
}
