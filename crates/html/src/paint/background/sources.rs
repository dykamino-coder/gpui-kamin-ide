//! Decode CSS background sources and resolve color images after text inheritance.

use super::{CACHE, CACHE_CAP, Source, decode, read_bytes, svg_fragment};
use crate::document::state::ValueCx;
use std::{collections::HashMap, sync::Mutex};

/// Ключ источника с учётом `image-orientation` (css-images-3 §5.4).
///
/// Разворот по EXIF — часть САМОЙ картинки: после него у неё другой природный
/// размер, и кэш обязан различать развёрнутый растр и сырой. Отдельного
/// параметра у `source` нет намеренно: кэш ключуется строкой, и приставка
/// ключа дешевле, чем переписывание тринадцати мест вызова.
pub fn key_exif(src: &str, c: &crate::style::computed::Computed, vcx: ValueCx<'_>) -> String {
    let gradient = crate::style::computed::resolve_gradient_colors(
        src,
        c.color.unwrap_or(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }),
        vcx,
    );
    let src = gradient.as_ref();
    let resolved = crate::style::computed::image_function::arguments(src, vcx).and_then(|image| {
        let color = crate::style::values::color_space::resolve_relative(
            image.color?,
            c.color.unwrap_or(crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            vcx,
        )?;
        let color = color_image(color);
        Some(match image.raw_url {
            Some(url) => format!(
                "image({url}, {})",
                crate::style::computed::parse_image_color(&color, vcx)?
            ),
            None => color,
        })
    });
    let src = resolved.as_deref().unwrap_or(src);
    if c.image_orient_none == Some(true) {
        format!("exif-none|{src}")
    } else {
        src.to_string()
    }
}

pub fn key(src: &str, c: &crate::style::computed::Computed, vcx: ValueCx<'_>) -> String {
    key_exif(src, c, vcx)
}

/// Разобрать ссылку в источник картинки; результат запоминается.
pub fn source(src: &str, vcx: ValueCx<'_>) -> Option<Source> {
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
    } else if let Some(image) = crate::style::computed::ImageSource::parse(src_plain, vcx) {
        computed_source(&image, orient)
    } else {
        load_url(src_plain, orient)
    };
    if matches!(
        found,
        Some(Source::Raster(_) | Source::Vector { .. } | Source::Shape { .. }) | None
    ) && let Ok(mut map) = cache.lock()
    {
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

/// Resource selection from computed operands never re-enters a CSS parser.
pub(crate) fn computed_source(
    image: &crate::style::computed::ImageSource,
    orient: bool,
) -> Option<Source> {
    use crate::style::computed::ImageSource;
    match image {
        ImageSource::Url(url) => load_url(url, orient),
        ImageSource::Invalid => None,
        ImageSource::Fallback { url, color } => load_url(url, orient).or_else(|| {
            color.as_ref().map(|c| Source::Gradient {
                image: std::sync::Arc::new(ImageSource::Color(c.clone())),
            })
        }),
        ImageSource::Filter { image, program } => Some(Source::Filtered {
            image: Box::new(computed_source(image, orient)?),
            list: program.clone(),
        }),
        _ => Some(Source::Gradient {
            image: std::sync::Arc::new(image.clone()),
        }),
    }
}

fn load_url(src: &str, orient: bool) -> Option<Source> {
    let key = if orient {
        src.to_owned()
    } else {
        format!("exif-none|{src}")
    };
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(found) = cache.lock().ok().and_then(|map| map.get(&key).cloned()) {
        return found;
    }
    let found = decode_url(src, orient);
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(key, found.clone());
    }
    found
}

fn decode_url(src: &str, orient: bool) -> Option<Source> {
    // Фрагмент адреса рисунка — его `<view>` (SVG 2 §8.2 «Linking into
    // SVG content»: `file.svg#id` показывает вид с тем `viewBox`): без
    // разбора путь с `#` не читался вовсе, и фон пропадал
    // (`background-size-cover-svg-view`, `-contain-svg-view`).
    let (path, view) = match src.split_once('#') {
        Some((p, f)) if !src.starts_with("data:") => (p, Some(f)),
        _ => (src, None),
    };
    read_bytes(path)
        .map(|b| match view {
            Some(id) => svg_fragment::rewrite_svg_fragment(b, id),
            None => b,
        })
        .as_deref()
        .and_then(|b| decode(b, orient))
}

#[cfg(test)]
mod tests {
    //! URL decoding takes precedence over a dimensionless image() color fallback.

    use super::{Source, key, source};
    use crate::style::{computed::Computed, values::value::Color};

    #[test]
    fn decode_url_before_using_fallback() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let image = source("image('data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%222%22 height=%223%22/%3E', red)", vcx).unwrap();
        assert!(matches!(image, Source::Vector { .. }));
        let size = image.intrinsic();
        assert_eq!((size.w, size.h), (Some(2.0), Some(3.0)));
        for url in [
            "data:text/plain,not-an-image",
            "missing-image-fallback-unit.png",
        ] {
            let image = source(&format!("image('{url}', green)"), vcx).unwrap();
            assert!(matches!(image, Source::Gradient { .. }));
            let size = image.intrinsic();
            assert_eq!((size.w, size.h, size.ratio), (None, None, None));
            assert!(source(&format!("image('{url}')"), vcx).is_none());
        }
    }

    #[test]
    fn fallback_current_color_resolves_after_inheritance_with_alpha() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let c = Computed {
            color: Color::parse("rgba(0, 128, 0, 0.5)", vcx),
            ..Computed::default()
        };
        let src = key("image('data:text/plain,invalid', currentColor)", &c, vcx);
        let Some(Source::Gradient { image }) = source(&src, vcx) else {
            panic!("fallback missing")
        };
        assert_eq!(
            match image.as_ref() {
                crate::style::computed::ImageSource::Color(c) => Some(c.value),
                _ => None,
            },
            c.color
        );
    }
}
