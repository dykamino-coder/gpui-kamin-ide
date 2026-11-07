//! Продолжения prepaint и paint встроенного HTML внутри SVG.
use super::*;
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};
pub(super) struct Foreign {
    pub(super) tree: Option<Element>,
    pub(super) opts: RenderOpts,
    pub(super) values: crate::values::ValueDoc,
    pub(super) size: (f32, f32),
}
impl Hooks for Foreign {
    type Layout = ();
    type Prepaint = Option<(Element, Vec<layer::Layer>)>;
    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            size: gpui::size(gpui::relative(1.).into(), gpui::relative(1.).into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }
    async fn prepaint_async(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        let mut tree = self.tree.take().expect("foreignObject prepaint once");
        let zoom = tree.style.used().inh.font.zoom_eff.unwrap_or(1.0);
        let (w, h) = self.size;
        let viewport =
            crate::dom::resource_facts::view_box_ratio(&tree).unwrap_or((w / zoom, h / zoom));
        let scale = (w / viewport.0).max(h / viewport.1);
        let mut layers = Vec::new();
        layer::prepare_async(
            &mut tree,
            &self.opts.root_style(),
            &self.opts,
            viewport,
            scale,
            window,
            cx,
            &mut layers,
            stack,
        )
        .await;
        Some((tree, layers))
    }
    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut (),
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, layout, prepaint, window, cx);
    }
    fn paint_async<'a>(
        &'a mut self,
        mut bounds: Bounds<Pixels>,
        _: &'a mut (),
        prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            let (mut tree, layers) = prepaint.take().expect("foreignObject paint after prepaint");
            let vcx = self.values.vcx();
            for mut layer in layers {
                let uri = match layer.cached.take() {
                    Some(uri) => Some(uri),
                    None => window
                        .paint_scene_to_png_async(layer.size, layer.density, async |window| {
                            if let Some(element) = &mut layer.element {
                                element.paint_async(window, cx, stack).await;
                            }
                        })
                        .await
                        .ok()
                        .map(|png| layer::cache(layer.key, &png, vcx)),
                };
                if let Some(uri) = uri {
                    layer::set_image(&mut tree, layer.node_id, uri);
                }
            }
            let (w, h) = self.size;
            let markup = super::super::serialize::serialize_sized(&tree, w, h, 0.0, 0.0, vcx);
            if let Some(image) = super::super::raster::rasterize(&markup, w, h, vcx) {
                bounds.size = gpui::size(px(w), px(h));
                super::super::device_image::paint(window, bounds, image);
            }
        }
    }
}
