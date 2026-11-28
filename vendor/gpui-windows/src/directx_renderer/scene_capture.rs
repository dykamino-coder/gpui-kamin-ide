//! Transparent offscreen targets for embedded document scenes.
use super::*;

impl DirectXRenderer {
    pub(crate) fn rasterize_scene(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        self.ensure_drawable_resources()?;
        let (width, height) = (size.width.0.max(1) as u32, size.height.0.max(1) as u32);
        anyhow::ensure!(
            width <= 8192 && height <= 8192,
            "scene capture exceeds texture limit"
        );
        let resources = self.res();
        let device = &self.dev().device;
        let (texture, view) = create_offscreen_target_and_its_view(device, width, height)?;
        let isolated = DirectXResources {
            swap_chain: resources.swap_chain.clone(),
            frame_gate: None,
            render_target: Some(texture),
            render_target_view: view,
            back_buffer: None,
            path_intermediate_srv: None,
            path_intermediate_msaa_texture: None,
            path_intermediate_msaa_view: None,
            viewport: D3D11_VIEWPORT {
                Width: width as f32,
                Height: height as f32,
                MaxDepth: 1.0,
                ..Default::default()
            },
        };
        let original = self.resources.replace(isolated);
        // Embedded captures have their own effect lifetime too. A plain capture
        // must not discard scratch or masks still used by the window's scene.
        let original_blur = std::mem::take(&mut self.blur);
        let dimensions = (self.width, self.height);
        (self.width, self.height) = (width, height);
        let result = self.render_to_image(scene, WindowBackgroundAppearance::Transparent);
        self.release_path_target();
        self.blur = original_blur;
        self.resources = original;
        (self.width, self.height) = dimensions;
        unsafe {
            self.dev()
                .device_context
                .OMSetRenderTargets(Some(slice::from_ref(&self.res().render_target_view)), None);
        }
        result
    }
}
