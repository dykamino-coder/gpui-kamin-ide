//! KaminIDE patch: the path intermediate belongs to the current target and
//! exists only while its scene can use it. Allocation follows a nonempty batch;
//! retention follows the complete scene, never the frame's damage rectangle.

use super::*;

impl DirectXRenderer {
    pub(super) fn ensure_path_target(&mut self) -> Result<()> {
        if self.res().path_intermediate_msaa_texture.is_some() {
            return Ok(());
        }
        // Construct all three COM owners before publishing any of them.
        let (texture, view, srv) = create_path_intermediate_msaa_texture_and_view(
            &self.dev().device,
            self.width,
            self.height,
        )?;
        let bytes = self
            .gpu_timer
            .instrumentation
            .config()
            .gpu_timing
            .then(|| gpu_memory::texture_bytes(&texture));
        let resources = self.resources.as_mut().context("resources missing")?;
        resources.path_intermediate_msaa_texture = Some(texture);
        resources.path_intermediate_msaa_view = view;
        resources.path_intermediate_srv = srv;
        if let Some(bytes) = bytes {
            self.gpu_timer
                .instrumentation
                .batches
                .path_target_created(bytes);
        }
        Ok(())
    }

    pub(super) fn release_path_target(&mut self) {
        let Some(resources) = self.resources.as_mut() else {
            return;
        };
        if resources.path_intermediate_msaa_texture.take().is_some() {
            resources.path_intermediate_msaa_view = None;
            resources.path_intermediate_srv = None;
            if self.gpu_timer.instrumentation.config().gpu_timing {
                self.gpu_timer.instrumentation.batches.path_target_dropped();
            }
        }
    }

    pub(super) fn release_unused_path_target(&mut self, scene: &Scene) {
        // Scenes are mutable between frames, including nested group contents.
        // Damage that misses paths must not throw away a reusable intermediate.
        fn has_paths(scene: &Scene) -> bool {
            !scene.paths.is_empty() || scene.groups.iter().any(|g| has_paths(&g.scene))
        }
        if self.res().path_intermediate_msaa_texture.is_some() && !has_paths(scene) {
            self.release_path_target();
        }
    }
}
