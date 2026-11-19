//! Prepare groups and instance buffers once, then replay disjoint scissors.
use super::super::*;

impl DirectXRenderer {
    pub(in crate::directx_renderer) fn needed_damage_groups(
        scene: &Scene,
        regions: Option<&[Region]>,
    ) -> Option<Vec<bool>> {
        regions.map(|regions| {
            let mut needed = vec![false; scene.groups.len()];
            for r in regions {
                for (n, hit) in needed
                    .iter_mut()
                    .zip(scene.groups_needed([r.x, r.y, r.width, r.height]))
                {
                    *n |= hit;
                }
            }
            needed
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
        self.upload_scene_buffers(scene)?;
        let ctx = self.dev().device_context.clone();
        for &region in regions {
            let restore = self.begin_scissor(region)?;
            self.backdrop_pending = self.backdrop.is_some();
            let drawn = self
                .clear_frame(clear)
                .and_then(|()| self.draw_scene_at(scene, Some([0; 6])));
            unsafe { ctx.RSSetState(&restore) };
            drawn?;
        }
        Ok(())
    }
}
