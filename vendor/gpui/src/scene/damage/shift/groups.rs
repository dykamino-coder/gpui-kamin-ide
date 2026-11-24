//! KaminIDE patch: the groups of a scroll step matched with the last frame's
//! groups moved by the step.
//!
//! A group's image is its scene rasterized into its region. When the scene,
//! every place in it and the region all move by the same whole device pixels,
//! the image is the same pixels: the renderer keeps it (`group_cache`) and the
//! frame comparison finds the group's composite equal to the moved one
//! (`compare.rs`). So a scroll step over filtered, blended or clipped boxes
//! neither redraws their images nor the window around them. WebRender keeps
//! picture surfaces per spatial node for the same reason; here the match is by
//! content, so a step that moves part of the groups keeps all of them that moved.

use super::super::{Area, KIND_SHIFT, bits, paint_order};
use super::compare::{Moved, at};
use super::items::Item;
use crate::scene::{PaintGroup, PaintSurface, Scene};
use crate::{ContentMask, Point, ScaledPixels};
use collections::FxHashMap;

#[cfg(test)]
mod tests;

/// Values a group shares with its match moved by `d` (`group_match::key`
/// with the place moved).
fn key(g: &PaintGroup, d: (f32, f32)) -> [u32; 13] {
    let (b, s) = (g.bounds, &g.scene);
    [
        (b.origin.x.0 + d.0).to_bits(),
        (b.origin.y.0 + d.1).to_bits(),
        b.size.width.0.to_bits(),
        b.size.height.0.to_bits(),
        g.blur_radius.to_bits(),
        s.shadows.len() as u32,
        s.quads.len() as u32,
        s.paths.len() as u32,
        s.monochrome_sprites.len() as u32,
        s.subpixel_sprites.len() as u32,
        s.polychrome_sprites.len() as u32,
        s.underlines.len() as u32,
        s.surfaces.len() as u32,
    ]
}

/// A group whose image a move can keep: no plane projection, no fragments,
/// no mask box in device pixels, and a scene without layers or backdrops.
fn movable(g: &PaintGroup) -> bool {
    g.projection.is_none()
        && g.fragments.is_empty()
        && g.mask_clip.is_none()
        && g.scene.book.layers.is_empty()
        && g.scene.hoisted_backdrops.is_empty()
}

/// For the groups of `new` without a match in `base` (old group plus one),
/// the group of `old` that paints the same image moved by `d`; marks them in
/// `moved` and returns the moved matches alone (old plus one, 0: none).
pub(in crate::scene::damage) fn match_moved(new: &Scene, old: &Scene, d: (i32, i32), size: (u32, u32), base: &mut [u32], moved: &mut Vec<bool>) -> Vec<u32> {
    moved.clear();
    moved.resize(new.groups.len(), false);
    let mut map = vec![0u32; new.groups.len()];
    let d = (d.0 as f32, d.1 as f32);
    let mut taken = vec![false; old.groups.len()];
    for &o in base.iter().filter(|&&o| o > 0) {
        taken[o as usize - 1] = true;
    }
    let mut buckets: FxHashMap<[u32; 13], Vec<u32>> = FxHashMap::default();
    for (o, g) in old.groups.iter().enumerate().filter(|(o, g)| !taken[*o] && movable(g)) {
        buckets.entry(key(g, d)).or_default().push(o as u32);
    }
    // Nested groups come first (`Scene::groups`): an enclosing group's
    // composites of them are judged by their matches made already.
    for (g, group) in new.groups.iter().enumerate() {
        if base[g] != 0 || !movable(group) {
            continue;
        }
        let Some(bucket) = buckets.get(&key(group, (0.0, 0.0))) else { continue };
        let found = bucket
            .iter()
            .copied()
            .find(|&o| !taken[o as usize] && same_group_moved(group, &old.groups[o as usize], d, size, &map));
        if let Some(o) = found {
            taken[o as usize] = true;
            map[g] = o + 1;
            base[g] = o + 1;
            moved[g] = true;
        }
    }
    map
}

/// Group `a` paints the image of group `b` moved by `d`.
fn same_group_moved(a: &PaintGroup, b: &PaintGroup, d: (f32, f32), size: (u32, u32), map: &[u32]) -> bool {
    let moved_point = |p: &Point<ScaledPixels>| Point { x: ScaledPixels(p.x.0 + d.0), y: ScaledPixels(p.y.0 + d.1) };
    a.bounds == at(b.bounds, d)
        && a.blur_radius.to_bits() == b.blur_radius.to_bits()
        && a.blend == b.blend
        && a.polygon_inverse == b.polygon_inverse
        && a.polygon.len() == b.polygon.len()
        && a.polygon.iter().zip(&b.polygon).all(|(p, q)| *p == moved_point(q))
        && match (&a.mask, &b.mask) {
            (None, None) => true,
            (Some(x), Some(y)) => x.id == y.id,
            _ => false,
        }
        && (a.mask.is_none() || a.mask_bounds == at(b.mask_bounds, d))
        && a.mask_once == b.mask_once
        && same_scene_moved(&a.scene, &b.scene, d, image_area(a, size), map)
}

/// Where group `g`'s image can have pixels: its bounds with more than the
/// renderer's region margin (blur spread and edge pixels), within the window.
/// Masks decide pixels at their centres, so only their parts in it matter.
fn image_area(g: &PaintGroup, size: (u32, u32)) -> Area {
    let m = 8.0 * g.blur_radius.max(0.0) + 8.0;
    let (o, s) = (g.bounds.origin, g.bounds.size);
    [
        (o.x.0 - m).max(0.0),
        (o.y.0 - m).max(0.0),
        (o.x.0 + s.width.0 + m).min(size.0 as f32),
        (o.y.0 + s.height.0 + m).min(size.1 as f32),
    ]
}

/// Scene `a` paints within `w` what `b` paints there moved by `d`: the same
/// primitives in the same order, each moved, masks cut by `w`.
fn same_scene_moved(a: &Scene, b: &Scene, d: (f32, f32), w: Area, map: &[u32]) -> bool {
    let (mut ea, mut eb) = (Vec::new(), Vec::new());
    paint_order(a, |e| ea.push(e));
    paint_order(b, |e| eb.push(e));
    if ea.len() != eb.len() {
        return false;
    }
    let cmp = Moved { d, w, exact: FxHashMap::default(), cell: None, groups: map, labels: Some((a, b)) };
    let item = |scene, entry| Item { scene, entry, area: w, marker: 0 };
    ea.iter().zip(&eb).all(|(&x, &y)| x >> KIND_SHIFT == y >> KIND_SHIFT && cmp.exact_pair(item(a, x), item(b, y)))
}

/// Composite `a` puts its image where `b`'s moved by `d` lands (`cut` and
/// `shift` are the comparison's mask cuts of the new and the moved old mask).
pub(super) fn same_surface_moved(
    a: &PaintSurface,
    b: &PaintSurface,
    d: (f32, f32),
    cut: impl Fn(&mut ContentMask<ScaledPixels>),
    shift: impl Fn(&mut ContentMask<ScaledPixels>),
) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = (a, b, d, cut, shift);
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (mut ma, mut mb) = (a.content_mask.clone(), b.content_mask.clone());
        cut(&mut ma);
        shift(&mut mb);
        a.bounds == at(b.bounds, d)
            && ma == mb
            && a.corner_radii == b.corner_radii
            && a.blur_radius.to_bits() == b.blur_radius.to_bits()
            && a.opacity.to_bits() == b.opacity.to_bits()
            && bits(a.color_matrix) == bits(b.color_matrix)
            && a.fragment == b.fragment
    }
}
