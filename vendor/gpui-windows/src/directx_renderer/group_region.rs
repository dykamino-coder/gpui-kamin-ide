//! KaminIDE patch: целочисленные области групп и ограниченный пул совместимых текстур.

use super::*;

#[path = "group_visible.rs"]
mod visible;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub(super) fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub(super) fn clipped(b: Bounds<ScaledPixels>, margin: f32, width: u32, height: u32) -> Self {
        let v = [
            b.origin.x.0,
            b.origin.y.0,
            b.size.width.0,
            b.size.height.0,
            margin,
        ];
        if !v.into_iter().all(f32::is_finite) {
            return Self::full(width, height);
        }
        let x = (b.origin.x.0 - margin).floor().clamp(0.0, width as f32) as u32;
        let y = (b.origin.y.0 - margin).floor().clamp(0.0, height as f32) as u32;
        let right = (b.origin.x.0 + b.size.width.0 + margin)
            .ceil()
            .clamp(x as f32, width as f32) as u32;
        let bottom = (b.origin.y.0 + b.size.height.0 + margin)
            .ceil()
            .clamp(y as f32, height as f32) as u32;
        Self {
            x: x.min(width.saturating_sub(1)),
            y: y.min(height.saturating_sub(1)),
            width: (right - x).max(1),
            height: (bottom - y).max(1),
        }
    }

    /// The two rectangles share a pixel.
    pub(super) fn meets(self, o: Self) -> bool {
        self.x < o.x + o.width
            && o.x < self.x + self.width
            && self.y < o.y + o.height
            && o.y < self.y + self.height
    }

    pub(super) fn floats(self) -> [f32; 4] {
        [
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
        ]
    }

    pub(super) fn for_group(group: &gpui::PaintGroup, width: u32, height: u32) -> [Self; 2] {
        let margin = if group.blur_radius > 0.0 {
            gaussian::margin(group.blur_radius)
        } else {
            0.0
        };
        // Source uses raster space; the projection maps the frame into it.
        let bounds = visible::source(group, width, height);
        let raw = Self::clipped(bounds, margin + 2.0, width, height);
        [raw, Self::clipped(group.bounds, margin, width, height)]
    }
}

/// KaminIDE patch: a group composite's clip and geometry cut to what its
/// image can cover (`footprint`). A plane mark is placed under the content
/// mask of its 3D context, about the whole window
/// (`Window::insert_plane_mark`), and the composite pass ran over all of it:
/// on WARP each face of a cube cost a window of pixels. The pixels
/// left out sample no source and take no colour, and the cut edges are whole
/// pixels, so the frame is the same; cc and WebRender bound a render
/// surface's composite the same way, by its content's projected bounds
/// within the clip, not by the viewport.
pub(super) fn narrow_composite(q: &mut BlurQuad, vw: f32, vh: f32) {
    let Some(f) = visible::footprint(q, vw, vh) else {
        return;
    };
    // The clip keeps the pixels; the bounds are the geometry rasterized (a
    // clip distance discards pixels only after WARP walked them). Rounded
    // bounds shade their own edge, so only a square composite (a plane's)
    // moves them: its edge then lies 2 px beyond every pixel with a source.
    cut_to(&mut q.content_mask, f);
    if q.corner_radii.iter().all(|&r| r == 0.0) {
        cut_to(&mut q.bounds, f);
    }
}

/// `r` (x, y, w, h) cut to `f` (x0, y0, x1, y1), an edge it does not move
/// keeping its float; empty: zero width.
fn cut_to(r: &mut [f32; 4], f: [f32; 4]) {
    for (k, (lo, hi)) in [(f[0], f[2]), (f[1], f[3])].into_iter().enumerate() {
        let end = r[k] + r[k + 2];
        let (start, stop) = (r[k].max(lo), end.min(hi));
        if start >= stop {
            r[2] = 0.0;
            return;
        }
        if start != r[k] || stop != end {
            (r[k], r[k + 2]) = (start, stop - start);
        }
    }
}

impl DirectXRenderer {
    /// Frame rectangle a group composite can write (and whose backdrop it reads).
    pub(super) fn composite_region(&self, s: &PaintSurface) -> Region {
        let target = Region::full(self.width, self.height);
        let mut clipped = s.bounds.intersect(&s.content_mask.bounds);
        if s.group > 0 {
            if let Some((quad, _)) = self.group_composite_quad(s) {
                let [x, y, w, h] = quad.content_mask;
                let mask = Bounds::new(point(ScaledPixels(x), ScaledPixels(y)), size(ScaledPixels(w), ScaledPixels(h)));
                clipped = visible::output(&quad, clipped.intersect(&mask));
            }
        }
        let region = Region::clipped(clipped, 0.0, self.width, self.height);
        let x = region.x.max(target.x).min(target.x + target.width - 1);
        let y = region.y.max(target.y).min(target.y + target.height - 1);
        let right = (region.x + region.width)
            .min(target.x + target.width)
            .max(x + 1);
        let bottom = (region.y + region.height)
            .min(target.y + target.height)
            .max(y + 1);
        Region {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    pub(super) fn copy_group_backdrop(&mut self, s: &PaintSurface) -> Result<[f32; 4]> {
        let target = Region::full(self.width, self.height);
        let region = self.composite_region(s);
        let (x, y, right, bottom) = (
            region.x,
            region.y,
            region.x + region.width,
            region.y + region.height,
        );
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        if self
            .blur
            .blend_copy
            .as_ref()
            .is_none_or(|t| t.width != region.width || t.height != region.height)
        {
            let texture = self.blur.pool.take(&device, region.width, region.height)?;
            if let Some(old) = self.blur.blend_copy.replace(texture) {
                self.blur.pool.put(old);
            }
        }
        let copy = self.blur.blend_copy.as_ref().unwrap();
        let source = self
            .group_target_tex
            .as_ref()
            .or(self.res().render_target.as_ref())
            .context("render target missing")?;
        let area = D3D11_BOX {
            left: x - target.x,
            top: y - target.y,
            front: 0,
            right: right - target.x,
            bottom: bottom - target.y,
            back: 1,
        };
        unsafe {
            dc.CopySubresourceRegion(&copy.texture, 0, 0, 0, 0, source, 0, Some(&area));
            dc.PSSetShaderResources(2, Some(&copy.srv));
        }
        Ok(region.floats())
    }

    /// Lay out the group atlas for `scene`; returns which groups already have
    /// their image in it (`group_cache`).
    pub(super) fn prepare_group_targets(&mut self, scene: &Scene) -> Result<Vec<bool>> {
        let device = self.dev().device.clone();
        self.blur.group_regions.clear();
        for group in &scene.groups {
            let regions = Region::for_group(group, self.width, self.height);
            self.blur
                .group_regions
                .push(regions[usize::from(group.blur_radius > 0.0)]);
        }
        if let Some((places, keep)) = self.keep_group_places(scene) {
            self.blur.group_place = places;
            return Ok(keep);
        }
        // KaminIDE patch: the retained images share atlas pages (`groups`).
        let sizes: Vec<(u32, u32)> = self
            .blur
            .group_regions
            .iter()
            .map(|r| (r.width, r.height))
            .collect();
        let (places, pages) = group_atlas::pack(&sizes);
        self.blur.group_place = places;
        for (k, &need) in pages.iter().enumerate() {
            if self
                .blur
                .groups
                .get(k)
                .is_some_and(|t| group_atlas::page_fits((t.width, t.height), need))
            {
                continue;
            }
            let (w, h) = group_atlas::page_size(need);
            let page = create_blur_texture(&device, w, h, true)?;
            if k < self.blur.groups.len() {
                self.blur.groups[k] = page;
            } else {
                self.blur.groups.push(page);
            }
        }
        self.blur.groups.truncate(pages.len());
        let textures: Vec<(u32, u32)> = self
            .blur
            .groups
            .iter()
            .map(|t| (t.width, t.height))
            .collect();
        self.retained.groups.layout = Some((
            group_atlas::Shelves::of_layout(&self.blur.group_place, &textures),
            textures.len(),
        ));
        Ok(vec![false; scene.groups.len()])
    }
}

#[cfg(test)]
mod tests {
    use super::Region;
    use gpui::{ScaledPixels, bounds, point, size};

    #[test]
    fn fractional_bounds_round_outward_and_margin_stays_inside_frame() {
        let b = bounds(
            point(ScaledPixels(31.25), ScaledPixels(-2.5)),
            size(ScaledPixels(180.5), ScaledPixels(140.25)),
        );
        assert_eq!(
            Region::clipped(b, 4.8, 800, 600),
            Region {
                x: 26,
                y: 0,
                width: 191,
                height: 143
            }
        );
        let outside = bounds(
            point(ScaledPixels(900.0), ScaledPixels(700.0)),
            size(ScaledPixels(20.0), ScaledPixels(10.0)),
        );
        assert_eq!(
            Region::clipped(outside, 4.8, 800, 600),
            Region {
                x: 799,
                y: 599,
                width: 1,
                height: 1
            }
        );
        assert_eq!(
            Region::clipped(b, f32::NAN, 800, 600),
            Region::full(800, 600)
        );
    }
}
