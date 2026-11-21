//! Prepare groups once and replay the scene once within the damage union.
use super::super::*;
use super::group_damage;

fn envelope(regions: &[Region]) -> Option<Region> {
    let first = *regions.first()?;
    Some(regions.iter().skip(1).fold(first, |a, b| {
        let (x, y) = (a.x.min(b.x), a.y.min(b.y));
        Region {
            x,
            y,
            width: (a.x + a.width).max(b.x + b.width) - x,
            height: (a.y + a.height).max(b.y + b.height) - y,
        }
    }))
}

impl DirectXRenderer {
    pub(in crate::directx_renderer) fn needed_damage_groups(
        scene: &Scene,
        regions: Option<&[Region]>,
    ) -> Option<Vec<bool>> {
        regions.map(|regions| {
            if let [r] = regions {
                return scene.groups_needed([r.x, r.y, r.width, r.height]);
            }
            let damage = group_damage::GroupDamage::new(regions);
            scene.groups_needed_where(|rect| damage.intersects(rect))
        })
    }

    pub(in crate::directx_renderer) fn draw_damage_regions(
        &mut self,
        scene: &Scene,
        regions: Option<&[Region]>,
        clear: [f32; 4],
    ) -> Result<()> {
        let Some(regions) = regions else {
            return self.draw_scene(scene);
        };
        let Some(within) = envelope(regions) else {
            return Ok(());
        };
        let ctx = self.dev().device_context.clone();
        let restore = self.begin_scissor(within)?;
        let (mut depth, mut reference) = (None, 0);
        unsafe { ctx.OMGetDepthStencilState(Some(&mut depth), Some(&mut reference)) };
        let drawn = (|| {
            // Group preparation may fail before restoring the frame viewport/buffers.
            self.restore_frame_globals()?;
            if regions.len() > 1 {
                let device = self.dev().device.clone();
                self.retained
                    .mask
                    .prepare(&device, self.width, self.height)?;
                self.retained.mask.active = true;
                self.bind_current_target();
                self.retained.mask.mark(&ctx);
                for &region in regions {
                    self.begin_scissor(region)?;
                    self.clear_frame(clear)?;
                }
                self.retained.mask.test(&ctx);
                self.begin_scissor(within)?;
            } else {
                self.clear_frame(clear)?;
            }
            self.blur.frame_within = Some(within);
            self.backdrop_pending = self.backdrop.is_some();
            self.draw_scene(scene)
        })();
        self.blur.frame_within = None;
        self.retained.mask.active = false;
        // Restore the depth state, rasterizer and target even when a pass fails.
        unsafe {
            ctx.OMSetDepthStencilState(depth.as_ref(), reference);
            ctx.RSSetState(&restore);
        }
        self.bind_current_target();
        drawn
    }
}
