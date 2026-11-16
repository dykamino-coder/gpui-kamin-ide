//! KaminIDE patch: group images kept in the group atlas between frames.
//!
//! A group of a frame compared with the last one (`Scene::group_base`) that
//! paints what a group of the last frame painted takes that group's atlas
//! rectangle and its finished image, whatever its index: an inserted, removed
//! or reordered node shifts the indices of later groups but not their images.
//! The other groups get free space on the shelves of the last full layout
//! (`group_atlas::Shelves`); when they do not fit, the atlas is packed again
//! and nothing is kept for that frame. Chromium caches an unchanged render
//! surface per node (`RenderSurfaceImpl::ShouldCacheRenderSurface`) and
//! WebRender keeps render-task results by key; here the key is the group's
//! whole content, compared exactly. The cache costs no memory: it is the
//! atlas the frame needs anyway, and a dead group's rectangle is free space.

use super::*;

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
        for (g, region) in regions.iter().enumerate() {
            let old = if same { Some(g) } else { scene.group_base(g) };
            let Some(o) = old.filter(|&o| {
                c.valid.get(o).copied().unwrap_or(false) && c.regions.get(o) == Some(region)
            }) else {
                continue;
            };
            if shelves.occupy(c.places[o], (region.width, region.height)) {
                places[g] = Some(c.places[o]);
                keep[g] = true;
            }
        }
        if !keep.contains(&true) {
            return None;
        }
        let places = places
            .into_iter()
            .zip(regions)
            .map(|(p, r)| p.or_else(|| shelves.alloc((r.width, r.height))))
            .collect::<Option<Vec<_>>>()?;
        Some((places, keep))
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
