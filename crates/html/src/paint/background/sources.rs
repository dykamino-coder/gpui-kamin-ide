//! Select computed image operands and decode resource URLs with shared caching.

use super::{CACHE, CACHE_CAP, Source, decode, read_bytes, svg_fragment};
use crate::document::state::ValueCx;
use std::{collections::HashMap, sync::Mutex};

/// Разобрать ссылку в источник картинки; результат запоминается.
pub fn source(src: &str, _vcx: ValueCx<'_>) -> Option<Source> {
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

/// Replaced CSS content retains its computed image; HTML src remains a resource URL.
pub(crate) fn element_source(
    src: &str,
    style: &crate::style::computed::Computed,
) -> Option<Source> {
    let orient = style.image_orient_none != Some(true);
    match style.content_image() {
        Some(image) => computed_source(image, orient),
        None => load_url(src, orient),
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

    use super::{Source, computed_source};
    use crate::style::computed::ImageSource;
    fn parsed(raw: &str, vcx: crate::document::state::ValueCx<'_>) -> Option<Source> {
        computed_source(&ImageSource::parse(raw, vcx)?, true)
    }
    use crate::style::{computed::Computed, values::value::Color};

    #[test]
    fn decode_url_before_using_fallback() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let image = parsed("image('data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%222%22 height=%223%22/%3E', red)", vcx).unwrap();
        assert!(matches!(image, Source::Vector { .. }));
        let size = image.intrinsic();
        assert_eq!((size.w, size.h), (Some(2.0), Some(3.0)));
        for url in [
            "data:text/plain,not-an-image",
            "missing-image-fallback-unit.png",
        ] {
            let image = parsed(&format!("image('{url}', green)"), vcx).unwrap();
            assert!(matches!(image, Source::Gradient { .. }));
            let size = image.intrinsic();
            assert_eq!((size.w, size.h, size.ratio), (None, None, None));
            assert!(parsed(&format!("image('{url}')"), vcx).is_none());
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
        let mut src =
            ImageSource::parse("image('data:text/plain,invalid', currentColor)", vcx).unwrap();
        src.resolve_colors(c.color.unwrap());
        let Some(Source::Gradient { image }) = computed_source(&src, true) else {
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
