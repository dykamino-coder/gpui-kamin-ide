//! Decode CSS background sources and resolve color images after text inheritance.

use super::{CACHE, CACHE_CAP, Source, decode, read_bytes, svg_fragment};
use std::{collections::HashMap, sync::Mutex};

/// Ключ источника с учётом `image-orientation` (css-images-3 §5.4).
///
/// Разворот по EXIF — часть САМОЙ картинки: после него у неё другой природный
/// размер, и кэш обязан различать развёрнутый растр и сырой. Отдельного
/// параметра у `source` нет намеренно: кэш ключуется строкой, и приставка
/// ключа дешевле, чем переписывание тринадцати мест вызова.
pub fn key_exif(src: &str, c: &crate::style::computed::Computed) -> String {
    let resolved = crate::style::computed::parse_image_color(src)
        .and_then(|color| {
            crate::style::values::color_space::resolve_relative(
                color,
                c.color.unwrap_or(crate::style::values::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
            )
        })
        .map(color_image);
    let src = resolved.as_deref().unwrap_or(src);
    if c.image_orient_none == Some(true) {
        format!("exif-none|{src}")
    } else {
        src.to_string()
    }
}

pub fn key(src: &str, c: &crate::style::computed::Computed) -> String {
    key_exif(src, c)
}

/// Разобрать ссылку в источник картинки; результат запоминается.
pub fn source(src: &str) -> Option<Source> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(src)
    {
        return hit.clone();
    }
    // Приставка снимается ДО чтения файла: читать надо настоящий адрес.
    let (orient, src_plain) = match src.strip_prefix("exif-none|") {
        Some(rest) => (false, rest),
        None => (true, src),
    };
    let found = if let Some(shape) = src_plain.strip_prefix("shape:") {
        Some(Source::Shape {
            raw: shape.to_string(),
        })
    } else if let Some(color) = crate::style::computed::parse_image_color(src_plain)
        .and_then(crate::style::values::value::Color::parse)
    {
        Some(Source::Gradient {
            raw: color_image(color),
        })
    } else if src.starts_with("linear-gradient(")
        || src.starts_with("radial-gradient(")
        || src.starts_with("conic-gradient(")
        // Повторяющиеся градиенты — те же записи (css-images-3 §4):
        // растеризатор ниже их понимает, а опознание пропускало
        // (`shape-outside-linear-gradient-004`).
        || src.starts_with("repeating-linear-gradient(")
        || src.starts_with("repeating-radial-gradient(")
        || src.starts_with("repeating-conic-gradient(")
        || src.starts_with("cross-fade(")
    {
        Some(Source::Gradient {
            raw: src.to_string(),
        })
    } else {
        // Фрагмент адреса рисунка — его `<view>` (SVG 2 §8.2 «Linking into
        // SVG content»: `file.svg#id` показывает вид с тем `viewBox`): без
        // разбора путь с `#` не читался вовсе, и фон пропадал
        // (`background-size-cover-svg-view`, `-contain-svg-view`).
        let (path, view) = match src_plain.split_once('#') {
            Some((p, f)) if !src_plain.starts_with("data:") => (p, Some(f)),
            _ => (src_plain, None),
        };
        read_bytes(path)
            .map(|b| match view {
                Some(id) => svg_fragment::resolve(b, id),
                None => b,
            })
            .as_deref()
            .and_then(|b| decode(b, orient))
    };
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(src.to_string(), found.clone());
    }
    found
}

/// Resolve currentColor without turning the color image into a CSS box fill.
fn color_image(c: crate::style::values::value::Color) -> String {
    let color = format!(
        "rgba({}, {}, {}, {})",
        c.r * 255.0,
        c.g * 255.0,
        c.b * 255.0,
        c.a
    );
    format!("image({color})")
}

/// Color image samples use straight alpha, as expected by GPUI's image shader.
pub(super) fn raster_color(raw: &str, w: u32, h: u32) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let color = crate::style::computed::parse_image_color(raw)
        .and_then(crate::style::values::value::Color::parse)?;
    let pixel = [
        (color.b * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.r * 255.0).round() as u8,
        (color.a * 255.0).round() as u8,
    ];
    let bytes = pixel
        .into_iter()
        .cycle()
        .take((w * h * 4) as usize)
        .collect();
    gpui::bgra_bytes_to_image(w, h, bytes)
}
