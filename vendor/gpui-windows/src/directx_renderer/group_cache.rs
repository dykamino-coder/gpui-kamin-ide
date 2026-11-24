//! KaminIDE patch: group images kept in the group atlas between frames.
//!
//! A group of a frame compared with the last one (`Scene::group_base`) that
//! paints what a group of the last frame painted takes that group's atlas
//! rectangle and its finished image, whatever its index: an inserted, removed
//! or reordered node shifts the indices of later groups but not their images.
//! The other groups get free space on the shelves of the last full layout
//! (`group_atlas::Shelves`); when they do not fit, the atlas is packed again
//! and nothing is kept for that frame. A group that moved with a scroll step
//! by whole pixels (`Scene::group_match`) keeps its image when its region
//! moved by the same step: the image is the same pixels at a new place.
//! Chromium caches an unchanged render
//! surface per node (`RenderSurfaceImpl::ShouldCacheRenderSurface`) and
//! WebRender keeps render-task results by key; here the key is the group's
//! whole content, compared exactly. The cache costs no memory: it is the
//! atlas the frame needs anyway, and a dead group's rectangle is free space.

use super::*;

/// `r` moved by `d` device pixels, when it stays at non-negative coordinates.
fn moved(r: Region, d: (i32, i32)) -> Option<Region> {
    let x = u32::try_from(r.x as i64 + d.0 as i64).ok()?;
    let y = u32::try_from(r.y as i64 + d.1 as i64).ok()?;
    Some(Region { x, y, ..r })
}

#[derive(Default)]
pub(super) struct GroupCache {
    /// Scene number whose groups `valid` describes.
    pub(super) id: u64,
    /// The device and atlas pages the images were drawn on: a kept image is
    /// valid only on the same objects (font gamma and contrast are fixed per
    /// process, `get_font_info`; atlas tiles change only with the epoch in
    /// `retained::Target`, which `chain` requires).
    pub(super) objects: Vec<usize>,
    pub(super) places: Vec<group_atlas::Place>,
    pub(super) regions: Vec<Region>,
    /// The atlas rectangle of group `i` holds its finished image.
    pub(super) valid: Vec<bool>,
    /// The empty shelves of the last full layout and its number of pages.
    pub(super) layout: Option<(group_atlas::Shelves, usize)>,
    /// Groups of the last frame drawn and kept (diagnostics and tests).
    pub(super) counts: GroupCounts,
    /// One kept image per frame drawn again and compared (`group_scrub`).
    pub(super) scrub: super::group_scrub::Scrub,
}

/// How the groups of the last frame got their images.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct GroupCounts {
    pub(super) kept: usize,
    pub(super) drawn: usize,
}

impl DirectXRenderer {
    /// Atlas places for `scene`'s groups (regions in `blur.group_regions`)
    /// that keep the images of the last frame: per group its place and
    /// whether its image is already there. None: pack the atlas again.
    pub(super) fn keep_group_places(&self, scene: &Scene) -> Option<(Vec<group_atlas::Place>, Vec<bool>)> {
        let c = &self.retained.groups;
        let (same, base) = self.retained.chain?;
        let (shelves, pages) = c.layout.as_ref()?;
        if c.id != base || *pages != self.blur.groups.len() || c.objects != self.group_objects() {
            return None;
        }
        let regions = &self.blur.group_regions;
        let mut shelves = shelves.clone();
        let mut places = vec![None; regions.len()];
        let mut keep = vec![false; regions.len()];
        // A matched group keeps its rectangle whether or not its image is
        // there: a frame drawn within a rectangle draws only the groups it
        // needs, and the others must not take new space on the packed shelves
        // (they would not fit, and the repack would drop every kept image).
        // Only a group without a match, or with another region, gets new space.
        let mut held = false;
        for (g, region) in regions.iter().enumerate() {
            let old = if same { Some((g, (0, 0))) } else { scene.group_match(g) };
            let Some((o, _)) = old.filter(|&(o, d)| c.regions.get(o).and_then(|r| moved(*r, d)) == Some(*region)) else {
                continue;
            };
            let Some(&place) = c.places.get(o) else {
                continue;
            };
            if shelves.occupy(place, (region.width, region.height)) {
                places[g] = Some(place);
                keep[g] = c.valid.get(o).copied().unwrap_or(false);
                held = true;
            }
        }
        if !held {
            return None;
        }
        let places = places
            .into_iter()
            .zip(regions)
            .map(|(p, r)| p.or_else(|| shelves.alloc((r.width, r.height))))
            .collect::<Option<Vec<_>>>()?;
        Some((places, keep))
    }

    /// A frame kept without drawing (`retained::Plan::Keep`) whose scene is a
    /// new one: its groups paint what their matches painted, so the images
    /// stay valid under the new scene's numbering. Without this the next frame,
    /// compared with the kept one, would find the cache describing an older
    /// scene and lose every image. A group without a match ends the carry.
    pub(super) fn carry_kept_groups(&mut self, scene: &Scene) {
        let Some((false, base)) = self.retained.chain else {
            return;
        };
        let c = &mut self.retained.groups;
        if c.id != base {
            return;
        }
        let mut carried = (Vec::new(), Vec::new(), Vec::new());
        for g in 0..scene.groups.len() {
            let Some((o, d)) = scene.group_match(g).filter(|&(o, _)| o < c.places.len().min(c.regions.len())) else {
                return;
            };
            let Some(region) = moved(c.regions[o], d) else {
                return;
            };
            carried.0.push(c.places[o]);
            carried.1.push(region);
            carried.2.push(c.valid.get(o).copied().unwrap_or(false));
        }
        (c.places, c.regions, c.valid) = carried;
        c.id = scene.frame_damage().2;
    }

    /// The atlas now holds the images of `scene`'s groups marked in `valid`.
    pub(super) fn groups_kept(&mut self, scene: &Scene, valid: Vec<bool>, kept: usize) {
        let objects = self.group_objects();
        let c = &mut self.retained.groups;
        c.id = scene.frame_damage().2;
        c.objects = objects;
        c.places.clone_from(&self.blur.group_place);
        c.regions.clone_from(&self.blur.group_regions);
        c.counts = GroupCounts {
            kept,
            drawn: valid.iter().filter(|&&v| v).count() - kept,
        };
        c.valid = valid;
    }

    /// Identity of the device and of every atlas page.
    fn group_objects(&self) -> Vec<usize> {
        use windows::core::Interface;
        std::iter::once(self.dev().device.as_raw() as usize)
            .chain(self.blur.groups.iter().map(|p| p.texture.as_raw() as usize))
            .collect()
    }
}
