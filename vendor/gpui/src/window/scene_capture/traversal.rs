//! Съёмка встроенного документа сохраняет единый стек обхода.
use super::*;
impl Window {
    /// Retain capture density until the embedded continuation completes.
    pub async fn with_scene_capture_scale_async<R>(&mut self, density: f32, f: impl AsyncFnOnce(&mut Self) -> R) -> R {
        let previous = self.scale_factor;
        self.scale_factor *= density.max(1.0);
        self.with_restore_async(previous, |window, previous| window.scale_factor = *previous, f).await.0
    }
    /// Capture embedded paint without creating a nested traversal driver.
    pub async fn paint_scene_to_png_async(&mut self, size: Size<Pixels>, density: f32, paint: impl AsyncFnOnce(&mut Self)) -> anyhow::Result<Vec<u8>> {
        self.invalidator.debug_assert_paint();
        let sf = self.scale_factor() * density.max(1.0);
        let (_, scene) = self.with_isolated_scene_async(async |window| {
            let previous = (mem::replace(&mut window.scale_factor, sf), window.mask_scale.take(), mem::replace(&mut window.element_opacity, 1.0));
            window.with_restore_async(previous, |window, previous| {
                window.scale_factor = previous.0;
                window.mask_scale = previous.1;
                window.element_opacity = previous.2;
            }, async |window| {
                window.with_absolute_transformation_async(TransformationMatrix::unit(), async |window| {
                    window.with_content_mask_replaced_async(ContentMask { bounds: Bounds { origin: Point::default(), size } }, paint).await;
                }).await;
            }).await;
        }).await;
        self.scene_png(scene, size, sf)
    }
}
