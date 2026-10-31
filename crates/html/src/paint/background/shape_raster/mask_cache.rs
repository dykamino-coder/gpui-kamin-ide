//! Кэш масок по физическому размеру и вычисленному контуру после CSS-нормализации.

use super::{Rrect, contour_coverage};
use crate::cache::FloatKey;
use crate::paint::cache::{BoundedCache, MIB, optional_image_bytes};
use gpui::RenderImage;
use std::sync::Arc;

#[derive(Hash, PartialEq, Eq)]
struct Key {
    size: (u32, u32),
    corners: [[FloatKey; 2]; 4],
    curvature: [FloatKey; 4],
    ring: Option<[FloatKey; 4]>,
}

static MASKS: BoundedCache<Key, Option<Arc<RenderImage>>> =
    BoundedCache::new(32 * MIB, optional_image_bytes);

pub(super) fn raster(r: &Rrect, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    let key = Key {
        size: (w, h),
        corners: r.corners.map(|(x, y)| [x.into(), y.into()]),
        curvature: r.k.map(FloatKey::from),
        ring: r.ring.map(|v| v.map(FloatKey::from)),
    };
    MASKS.get_or_insert_with(key, || uncached(r, w, h))
}

fn uncached(r: &Rrect, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    kamin_style::work::add!(RoundedMaskRasters, 1);
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let a = contour_coverage(x as f32 + 0.5, y as f32 + 0.5, w as f32, h as f32, r);
            let v = (a * 255.0).round() as u8;
            bytes.extend_from_slice(&[v, v, v, v]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

#[cfg(test)]
mod tests {
    use super::super::parse_rrect;
    use super::*;

    #[test]
    fn equivalent_normalized_radii_share_mask_and_exact_coverage() {
        let a = parse_rrect("12px 12px 12px 12px 12px 12px 12px 12px", 12.0, 12.0, 1.0).unwrap();
        let b = parse_rrect("50% 50% 50% 50% 50% 50% 50% 50%", 12.0, 12.0, 2.0).unwrap();
        let first = raster(&a, 12, 12).unwrap();
        let hit = raster(&b, 12, 12).unwrap();
        assert!(Arc::ptr_eq(&first, &hit));
        assert_eq!(first.as_bytes(0), uncached(&b, 12, 12).unwrap().as_bytes(0));
    }

    #[test]
    fn curvature_ring_and_device_size_do_not_alias() {
        let mut r = Rrect {
            corners: [(4.0, 3.0); 4],
            k: [1.0; 4],
            ring: None,
        };
        let base = raster(&r, 19, 13).unwrap();
        assert!(!Arc::ptr_eq(&base, &raster(&r, 20, 13).unwrap()));
        r.k[0] = -1.0;
        let shaped = raster(&r, 19, 13).unwrap();
        assert!(!Arc::ptr_eq(&base, &shaped));
        r.ring = Some([2.0; 4]);
        let ring = raster(&r, 19, 13).unwrap();
        assert_ne!(shaped.as_bytes(0), ring.as_bytes(0));
        assert_eq!(ring.as_bytes(0), uncached(&r, 19, 13).unwrap().as_bytes(0));
    }
}
