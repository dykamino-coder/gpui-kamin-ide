//! SVG-ресурсы масок документа: определения `<mask id>`, `<clipPath id>` и `<filter id>` для
//! ссылок `url(#id)` из `mask-image`/`clip-path`/`filter`. Реестр (`MaskResources`) принадлежит
//! документу и пересобирается до построения дерева (ссылка может стоять раньше определения);
//! отложенные группы держат неизменяемый снимок, а не общий реестр. Здесь же матрица 4×5
//! из SVG-фильтра для `backdrop-filter: url(#id)`.

pub mod element;
pub(super) mod mask_geometry;
pub(crate) use crate::resources::svg_document::masks::MaskDefinitions;

/// Prepare document definitions while deferred groups retain their previous set.
pub(crate) fn collect_mask_defs(
    nodes: &[crate::dom::Node],
    vcx: crate::document::state::ValueCx<'_>,
) {
    vcx.doc.masks.prepare(nodes, vcx);
}

/// SVG `<filter>` для `backdrop-filter: url(#id)` → матрица 4×5 над
/// НЕумноженным RGBA (строки R, G, B, A: четыре множителя и сдвиг — как
/// `Filter::color_matrix`). Только ОДИН примитив с аффинной формулой:
/// `feColorMatrix type="matrix"` (20 чисел; filter-effects-1 Overview.bs:950)
/// или `feComponentTransfer` с `identity`/`linear`/`table` из двух значений
/// (Overview.bs:1159-1168: C' = v0 + C·(v1 − v0); пустой список — тождество,
/// Overview.bs:1197). И только при `color-interpolation-filters="sRGB"`:
/// начальное `linearRGB` (Overview.bs:614) делает формулу нелинейной в sRGB
/// кадра. Остальное — None: подложка не рисуется, как прежде.
pub(crate) fn svg_filter_matrix(def: &str) -> Option<[f32; 20]> {
    fn attr(tag: &str, name: &str) -> Option<String> {
        let head = &tag[..tag.find('>')?];
        let key = format!(" {name}=\"");
        let at = head.find(&key)? + key.len();
        let rest = &head[at..];
        Some(rest[..rest.find('"')?].trim().to_string())
    }
    fn nums(s: &str) -> Option<Vec<f32>> {
        s.split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter(|t| !t.is_empty())
            .map(|t| t.parse::<f32>().ok())
            .collect()
    }
    // Разметка `svg::write_element`: атрибуты ` имя="значение"`; регистр
    // имён тегов и атрибутов сводится к нижнему.
    let d = def.to_ascii_lowercase();
    if !d.contains("color-interpolation-filters=\"srgb\"") {
        return None;
    }
    let prims: Vec<&str> = d
        .match_indices("<fe")
        .map(|(i, _)| &d[i..])
        .filter(|s| !s.starts_with("<fefunc"))
        .collect();
    let [p] = prims.as_slice() else {
        return None;
    };
    let p: &str = p;
    let mut m = [
        1.0f32, 0.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    if p.starts_with("<fecolormatrix") {
        if attr(p, "type").is_some_and(|t| t != "matrix") {
            return None;
        }
        let v = nums(&attr(p, "values")?)?;
        if v.len() != 20 {
            return None;
        }
        m.copy_from_slice(&v);
    } else if p.starts_with("<fecomponenttransfer") {
        let body = &p[..p.find("</fecomponenttransfer").unwrap_or(p.len())];
        for (row, ch) in ['r', 'g', 'b', 'a'].into_iter().enumerate() {
            let Some(at) = body.find(&format!("<fefunc{ch}")) else {
                continue;
            };
            let f = &body[at..];
            let (slope, intercept) = match attr(f, "type").as_deref() {
                Some("identity") => continue,
                Some("table") => {
                    let v = nums(attr(f, "tablevalues").as_deref().unwrap_or(""))?;
                    match v.as_slice() {
                        [] => continue,
                        [v0, v1] => (v1 - v0, *v0),
                        _ => return None,
                    }
                }
                Some("linear") => (
                    attr(f, "slope")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(1.0),
                    attr(f, "intercept")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.0),
                ),
                _ => return None,
            };
            m[row * 5 + row] = slope;
            m[row * 5 + 4] = intercept;
        }
    } else {
        return None;
    }
    Some(m)
}

#[cfg(test)]
mod tests;
