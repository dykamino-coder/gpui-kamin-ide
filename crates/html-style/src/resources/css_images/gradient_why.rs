//! Why a gradient tile lookup missed, for the DevTools `why` journal: a key
//! seen before with another size or sampling, the same size with another
//! image (which field), or a first lookup. Runs only for misses while the
//! journal records; it keeps the keys of those misses, bounded.

use super::raster_keys::ImageKey;
use crate::cache::FloatKey;
use std::sync::Mutex;

/// The tile key's fields (`gradient_raster::TileKey`).
pub(super) type Fields = (ImageKey, u32, u32, [FloatKey; 2], bool);

/// Keys of past misses; past the bound the memory starts again.
const SEEN: usize = 4096;
static MISSED: Mutex<Vec<Fields>> = Mutex::new(Vec::new());

/// The detail of a miss of `key` for `reason` (`BoundedCache::lookup`).
pub(super) fn explain(reason: &'static str, key: &Fields) -> String {
    let mut seen = MISSED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let detail = detail(reason, key, &seen);
    if seen.len() >= SEEN {
        seen.clear();
    }
    seen.push(key.clone());
    detail
}

fn detail(reason: &'static str, key: &Fields, seen: &[Fields]) -> String {
    let (image, w, h, css, centers) = key;
    let at = format!("{w}x{h} tile");
    if reason != "new_key" {
        return format!("{at}: the same key was cached before ({reason})");
    }
    // The same image under other tile fields.
    if let Some((_, w0, h0, css0, c0)) = seen.iter().rev().find(|s| s.0 == *image) {
        let mut fields = Vec::new();
        if (w0, h0) != (w, h) {
            fields.push(format!("bitmap {w0}x{h0} -> {w}x{h}"));
        }
        if css0 != css {
            fields.push(format!("css size {css0:?} -> {css:?}"));
        }
        if c0 != centers {
            fields.push("pixel centers".into());
        }
        if fields.is_empty() {
            return format!("{at}: the same key missed before (cache cleared)");
        }
        return format!("{at}: key differs in {}", fields.join(", "));
    }
    // Another image at the same tile fields: which part of the image key.
    let same_tile = |s: &&Fields| (s.1, s.2, s.3, s.4) == (*w, *h, *css, *centers);
    if let Some(other) = seen.iter().rev().find(same_tile) {
        return format!(
            "{at}: first lookup of this image; nearest key at the same size differs in {}",
            image.differs(&other.0)
        );
    }
    format!("{at}: first lookup of this image")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(color: f32, w: u32) -> Fields {
        let c = FloatKey::from(color);
        (
            ImageKey::Color([c, c, c, c]),
            w,
            10,
            [FloatKey::from(10.0), FloatKey::from(10.0)],
            true,
        )
    }

    #[test]
    fn a_miss_names_the_field_that_differs() {
        let seen = vec![key(1.0, 10)];
        assert!(detail("new_key", &key(1.0, 20), &seen).contains("bitmap 10x10 -> 20x10"));
        assert!(detail("new_key", &key(0.5, 10), &seen).contains("differs in color"));
        assert!(detail("evicted", &key(1.0, 10), &seen).contains("(evicted)"));
        assert!(detail("new_key", &key(1.0, 10), &[]).ends_with("first lookup of this image"));
    }
}
