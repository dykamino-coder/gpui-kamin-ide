//! Mask sources preserve the element orientation when looking up cached images.

use super::Grouped;
use crate::document::state::ValueCx;

impl Grouped {
    pub(crate) fn mask_source(
        &self,
        source: &str,
        vcx: ValueCx<'_>,
    ) -> Option<crate::paint::background::Source> {
        // CSS Images 3 §5.1 applies image-orientation to decorative images too.
        // Keep oriented and encoded rasters (and their intrinsic sizes) distinct.
        if self.mask_orient_none {
            crate::paint::background::source(&format!("exif-none|{source}"), vcx)
        } else {
            crate::paint::background::source(source, vcx)
        }
    }
}

/// Голый источник слоя маски: содержимое `url(...)` либо запись градиента.
pub(super) fn mask_layer_source(layer: &str) -> Option<String> {
    let t = layer.trim();
    if t.contains("-gradient(") || t.starts_with("filter(") {
        return Some(t.to_string());
    }
    if t.starts_with("clipref:") || t.starts_with("pathdef:") || t.starts_with("shapedef:") {
        return Some(t.to_string());
    }
    let at = t.find("url(")?;
    let rest = &t[at + 4..];
    let end = rest.find(')')?;
    Some(
        rest[..end]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_string(),
    )
}

/// Содержимое `<mask id>`/`<clipPath id>` из ВНЕШНЕГО файла рисунка
/// (`mask-image: url(file.svg#id)`): грубый текстовый вырез — дерево
/// документа рисунка нам нигде больше не нужно. Второе — маскирует ли
/// определение АЛЬФОЙ: `<clipPath>` режет покрытием, `<mask>` — по своему
/// `mask-type` (css-masking-1 §7.2 `match-source`, светимость по умолчанию;
/// `mask-image-1c`: `mask-type="alpha"` у чёрных прямоугольников).
pub(super) fn svg_fragment(path: &str, id: &str) -> Option<(String, bool)> {
    let markup = std::fs::read_to_string(path).ok()?;
    for tag in ["mask", "clipPath"] {
        let mut rest = markup.as_str();
        while let Some(at) = rest.find(&format!("<{tag}")) {
            let head_end = rest[at..].find('>')? + at;
            let head = &rest[at..head_end];
            let close = format!("</{tag}>");
            let body_end = rest[head_end..].find(&close)? + head_end;
            if head.contains(&format!("id=\"{id}\"")) || head.contains(&format!("id='{id}'")) {
                let squeezed: String = head.chars().filter(|c| !c.is_whitespace()).collect();
                let alpha = tag == "clipPath"
                    || squeezed.contains("mask-type=\"alpha\"")
                    || squeezed.contains("mask-type='alpha'")
                    || squeezed.contains("mask-type:alpha");
                return Some((rest[head_end + 1..body_end].to_string(), alpha));
            }
            rest = &rest[body_end + close.len()..];
        }
    }
    None
}

/// Растр определения `<mask>`/`<clipPath>` из документа под коробку.
///
/// Содержимое сериализовано в контексте документа; маска берёт
/// светимость своих красок, обрезка — покрытие (заливка принудительно
/// белая), поэтому обе идут люминанс-растром.
pub(super) fn rasterize_mask_def(
    markup: &str,
    w: f32,
    h: f32,
    force_white: bool,
) -> Option<std::sync::Arc<gpui::RenderImage>> {
    // Внутри <clipPath> правило намотки несёт `clip-rule`; растеризатор
    // рисует контур как обычный и читает только `fill-rule`
    // (clip-path-shape-002: у эталона пропадала дырка evenodd).
    let markup = markup.replace("clip-rule", "fill-rule");
    let body = if force_white {
        format!(r##"<g fill="#ffffff">{markup}</g>"##)
    } else {
        markup
    };
    crate::svg::raster::rasterize(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">{body}</svg>"#
        ),
        w,
        h,
    )
}
