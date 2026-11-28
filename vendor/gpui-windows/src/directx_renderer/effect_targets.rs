//! Effect scratch belongs to the complete scene, independently of its damage.
//!
//! A retained group may need its scratch again after a mutation, so an unchanged
//! frame keeps it. Removing the last user releases it, including cached mask
//! uploads. No texture dimensions, sampling or paint operations change here.

use super::*;

#[derive(Default, Debug, PartialEq, Eq)]
struct Demand {
    groups: bool,
    backdrop: bool,
    cascade: bool,
    gaussian: bool,
    blend: bool,
}

impl Demand {
    fn scene(scene: &Scene) -> Self {
        let mut demand = Self::default();
        demand.visit(scene);
        demand
    }

    fn visit(&mut self, scene: &Scene) {
        for surface in &scene.surfaces {
            if surface.group == 0 {
                self.backdrop = true;
                self.cascade |= surface.color_matrix.is_none() || surface.blur_radius > 0.0;
            }
        }
        self.groups |= !scene.groups.is_empty();
        for group in &scene.groups {
            self.blend |= group.blend != 0;
            if group.blur_radius > 0.0 {
                match gaussian::direct_radius(group.blur_radius) {
                    Some(radius) => self.gaussian |= radius > gaussian::MAX_SINGLE_PASS_RADIUS,
                    None => self.cascade = true,
                }
            }
            self.visit(&group.scene);
        }
    }
}

impl DirectXRenderer {
    pub(super) fn release_unused_effect_targets(&mut self, scene: &Scene) {
        let demand = Demand::scene(scene);
        let instrumented = self.gpu_timer.instrumentation.config().gpu_timing;
        let (mut released, mut bytes) = (0, 0);
        let mut dropped = |texture: &ID3D11Texture2D| {
            released += 1;
            if instrumented {
                bytes += gpu_memory::texture_bytes(texture);
            }
        };
        let blur = &mut self.blur;
        let masks_dropped = blur.mask_cache.retain_scene(scene, &mut dropped);
        for (needed, target) in [
            (demand.groups, &mut blur.stage),
            (demand.backdrop, &mut blur.copy),
            (demand.gaussian, &mut blur.gauss),
            // Tile composites read a backdrop even with normal source-over.
            (
                demand.blend || (demand.groups && self.group_tiles),
                &mut blur.level_backdrop,
            ),
            (demand.blend, &mut blur.blend_copy),
        ] {
            if !needed && let Some(target) = target.take() {
                dropped(&target.texture);
            }
        }
        for (needed, targets) in [
            (demand.groups, &mut blur.groups),
            (demand.cascade, &mut blur.down),
        ] {
            if !needed {
                for target in targets.drain(..) {
                    dropped(&target.texture);
                }
            }
        }
        if !demand.blend {
            for target in blur.pool.idle() {
                dropped(&target.texture);
            }
            blur.pool = Default::default();
        }
        // Composite data is rebuilt before drawing, and is unused by a kept
        // frame. Its SRVs must not keep the previous scene's masks alive.
        if masks_dropped > 0 {
            blur.group_mask.clear();
        }
        if !demand.groups {
            blur.stage_dirty.clear();
            blur.layer.clear();
            blur.layer_instances = Default::default();
            blur.stage_rasterizer = None;
            blur.group_place.clear();
            blur.group_regions.clear();
            blur.group_slots.clear();
            blur.group_blend.clear();
            blur.group_poly.clear();
            blur.group_mask.clear();
            blur.group_clip.clear();
            blur.group_proj.clear();
            blur.group_frags.clear();
            blur.mask_cache.clear();
        }
        if released > 0 {
            // D3D's bindings own references too. The next pre_draw binds the
            // frame target and every pass supplies its shader resources again.
            let dc = &self.dev().device_context;
            unsafe {
                dc.PSSetShaderResources(0, Some(&[None, None, None, None]));
                dc.OMSetRenderTargets(None, None);
            }
            if instrumented {
                self.gpu_timer
                    .instrumentation
                    .batches
                    .effect_targets_dropped(released, bytes);
            }
        }
    }
}
