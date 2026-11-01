//! KaminIDE patch: целочисленные области групп и ограниченный пул совместимых текстур.

use super::*;

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
            (24.0 * (group.blur_radius / 14.0).clamp(0.2, 6.0)).min(96.0)
        } else {
            0.0
        };
        // Проекция читает исходные пиксели через H^-1, включая область вне рамки.
        let raw = if group.projection.is_some() {
            Self::full(width, height)
        } else {
            Self::clipped(group.bounds, margin + 2.0, width, height)
        };
        [raw, Self::clipped(group.bounds, margin, width, height)]
    }
}

impl DirectXRenderer {
    pub(super) fn copy_group_backdrop(&mut self, s: &PaintSurface) -> Result<[f32; 4]> {
        let target = Region::full(self.width, self.height);
        let clipped = s.bounds.intersect(&s.content_mask.bounds);
        let region = Region::clipped(clipped, 0.0, self.width, self.height);
        let x = region.x.max(target.x).min(target.x + target.width - 1);
        let y = region.y.max(target.y).min(target.y + target.height - 1);
        let right = (region.x + region.width)
            .min(target.x + target.width)
            .max(x + 1);
        let bottom = (region.y + region.height)
            .min(target.y + target.height)
            .max(y + 1);
        let region = Region {
            x,
            y,
            width: right - x,
            height: bottom - y,
        };
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

    pub(super) fn prepare_group_targets(&mut self, scene: &Scene) -> Result<()> {
        let device = self.dev().device.clone();
        self.blur.group_regions.clear();
        for group in &scene.groups {
            let regions = Region::for_group(group, self.width, self.height);
            self.blur
                .group_regions
                .push(regions[usize::from(group.blur_radius > 0.0)]);
        }
        for i in 0..self.blur.group_regions.len() {
            let r = self.blur.group_regions[i];
            if self
                .blur
                .groups
                .get(i)
                .is_some_and(|t| t.width == r.width && t.height == r.height)
            {
                continue;
            }
            let texture = self.blur.pool.take(&device, r.width, r.height)?;
            if i < self.blur.groups.len() {
                let old = std::mem::replace(&mut self.blur.groups[i], texture);
                self.blur.pool.put(old);
            } else {
                self.blur.groups.push(texture);
            }
        }
        while self.blur.groups.len() > self.blur.group_regions.len() {
            self.blur.pool.put(self.blur.groups.pop().unwrap());
        }
        Ok(())
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
