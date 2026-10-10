//! CSS `clip-path` на детях `<svg>`: синтез `<clipPath>` и опорные коробки фигур.

mod reference;
pub(super) use reference::reference_box;
pub(super) use reference::shape_box;
pub(super) use reference::translate_only;

use super::serialize::escape_attr;
use super::{IN_CLIP, VIEW_BOX};
use crate::dom::Element;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Конец функции `name(...)` с учётом вложенных скобок: индекс ПОСЛЕ ее `)`.
fn func_end(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Содержимое `<clipPath>` для CSS-фигуры в пользовательской системе фигуры;
/// `rb` — опорная коробка (x, y, w, h). `None` — фигура не выражается.
fn clip_body(
    c: &crate::style::computed::Computed,
    (bx, by, bw, bh): (f32, f32, f32, f32),
) -> Option<String> {
    use crate::style::values::value::Len;
    let at = |l: Len, side: f32| match l {
        Len::Px(v) => Some(v),
        Len::Pct(p) => Some(p * side),
        _ => None,
    };
    if let Some(points) = &c.clip_polygon {
        let mut pts = String::new();
        for (x, y) in points {
            pts.push_str(&format!("{},{} ", bx + at(*x, bw)?, by + at(*y, bh)?));
        }
        let rule = if c.clip_polygon_evenodd {
            "evenodd"
        } else {
            "nonzero"
        };
        return Some(format!(
            "<polygon clip-rule=\"{rule}\" points=\"{}\"/>",
            pts.trim_end()
        ));
    }
    if let Some([t, r, b, l]) = c.clip_inset {
        let (t, r, b, l) = (at(t, bh)?, at(r, bw)?, at(b, bh)?, at(l, bw)?);
        let (w, h) = ((bw - l - r).max(0.0), (bh - t - b).max(0.0));
        let round = c.clip_round.unwrap_or(0.0).max(0.0);
        return Some(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{h}\" rx=\"{round}\" ry=\"{round}\"/>",
            bx + l,
            by + t
        ));
    }
    if c.clip_bare_box {
        return Some(format!(
            "<rect x=\"{bx}\" y=\"{by}\" width=\"{bw}\" height=\"{bh}\"/>"
        ));
    }
    let spec = c.clip_shape.as_deref()?;
    if let Some(raw) = spec.strip_prefix("shape:") {
        if !(raw.starts_with("circle(") || raw.starts_with("ellipse(")) {
            return None;
        }
        // Хвост после функции — слово коробки (`… view-box`): отрезается.
        let func = &raw[..func_end(raw)?];
        let (cx, cy, rx, ry) = crate::paint::background::shape_params(func, bw, bh, 1.0)?;
        return Some(format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{rx}\" ry=\"{ry}\"/>",
            bx + cx,
            by + cy
        ));
    }
    let (rule, d) = if let Some(rest) = spec.strip_prefix("pathdef:") {
        let (rule, d) = rest.split_once(':')?;
        (rule, d.to_string())
    } else if let Some(rest) = spec.strip_prefix("shapedef:") {
        let (rule, body) = rest.split_once(':')?;
        (rule, crate::paint::background::shape_to_path(body, bw, bh)?)
    } else {
        return None;
    };
    // Точки `path()`/`shape()` отсчитываются от НАЧАЛА опорной коробки.
    let mut path = format!("<path clip-rule=\"{rule}\" transform=\"translate({bx} {by})\" d=\"");
    escape_attr(&d, &mut path);
    path.push_str("\"/>");
    Some(path)
}

/// CSS-обрезка базовой фигурой на SVG-ребёнке (css-masking-1 §5.1): usvg
/// понимает у `clip-path` только `url()`, поэтому фигура синтезируется
/// `<clipPath clipPathUnits="userSpaceOnUse">` ПЕРЕД элементом, а элемент
/// получает ссылку. Возвращает id синтезированного определения.
///
/// Прежде запись молча терялась, и фигура рисовалась целиком
/// (`svg-clip-path-fixed-values` 4.22, `clip-path-path-003` 1.05,
/// `svg-clip-path-ellipse-offset` 0.82, `clip-path-viewBox-1a/1b` 2.19/6.72;
/// близнецы `*-borderBox-1b`, `*-strokeBox-1b/1c` и родня держались под
/// порогом случайно — 0.47-0.49).
pub(super) fn synth_clip(e: &Element, out: &mut String) -> Option<String> {
    if e.tag.eq_ignore_ascii_case("svg") || IN_CLIP.with(|c| c.get()) {
        return None;
    }
    let has = |c: &crate::style::computed::Computed| {
        c.clip_polygon.is_some()
            || c.clip_inset.is_some()
            || c.clip_bare_box
            || c.clip_shape.as_deref().is_some_and(|s| {
                s.starts_with("shape:") || s.starts_with("pathdef:") || s.starts_with("shapedef:")
            })
    };
    // Каскад сильнее презентационного атрибута; атрибут разбирается тем же
    // `apply_one`, что и CSS-объявление.
    let parsed: crate::style::computed::Computed;
    let (c, view_box) = if has(&e.style) {
        (&e.style, false)
    } else {
        let raw = e
            .attr("clip-path")
            .filter(|v| !v.trim_start().starts_with("url("))?;
        let mut fresh = crate::style::computed::Computed::default();
        fresh.apply_one("clip-path", raw);
        parsed = fresh;
        (&parsed, raw.contains("view-box"))
    };
    if !has(c) {
        return None;
    }
    // Коробки SVG-элемента (css-masking-1): content/padding -> fill-box,
    // border/margin и умолчание -> stroke-box; `view-box` — начало системы
    // `viewBox`, его размер.
    let rb = if view_box {
        let (w, h) = VIEW_BOX.with(|v| v.get());
        (0.0, 0.0, w, h)
    } else {
        shape_box(e, !matches!(c.clip_ref, Some(2) | Some(3)))?
    };
    let body = clip_body(c, rb)?;
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    let id = format!("kamin-clip-{:x}", hasher.finish());
    out.push_str(&format!(
        "<clipPath id=\"{id}\" clipPathUnits=\"userSpaceOnUse\">{body}</clipPath>"
    ));
    Some(id)
}
