//! Resident CPU masks, including nested groups and retained layer contents.

use super::{Scene, SceneLayer};
use std::{collections::HashSet, rc::Rc};

enum Pending {
    Group(Rc<Scene>),
    Layer(Rc<SceneLayer>),
}

pub(super) fn report(root: &Scene, sink: &mut crate::heap_report::HeapSink) {
    let mut scenes = HashSet::new();
    let mut images = HashSet::new();
    let mut pending = Vec::new();
    let mut visit = |scene: &Scene, pending: &mut Vec<Pending>| {
        if !scenes.insert(scene as *const Scene) {
            return;
        }
        for group in &scene.groups {
            if let Some(mask) = &group.mask {
                if images.insert(mask.id) {
                    mask.heap_report(sink, "display-list/mask-images");
                }
            }
            pending.push(Pending::Group(group.scene.clone()));
        }
        pending.extend(scene.layer_contents().into_iter().map(Pending::Layer));
    };
    visit(root, &mut pending);
    while let Some(next) = pending.pop() {
        match next {
            Pending::Group(scene) => visit(&scene, &mut pending),
            Pending::Layer(layer) => visit(&layer.scene, &mut pending),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PaintGroup;

    #[test]
    fn nested_shared_masks_report_resident_pixels_once() {
        let mask = crate::bgra_bytes_to_image(3, 5, vec![255; 60]).unwrap();
        let mut inner = Scene::default();
        inner.groups.push(PaintGroup {
            mask: Some(mask.clone()),
            ..Default::default()
        });
        let group = PaintGroup {
            mask: Some(mask.clone()),
            scene: Rc::new(inner),
            ..Default::default()
        };
        let mut root = Scene::default();
        root.groups.extend([group.clone(), group]);
        let mut pixels = Vec::new();
        root.heap_report(&mut |path, address, bytes| {
            if address == mask.as_bytes(0).unwrap().as_ptr() {
                assert_eq!(path, "display-list/mask-images");
                pixels.push(bytes);
            }
        });
        assert_eq!(pixels, [60]);
    }

    #[test]
    fn retained_layer_reports_mask_without_decoding_lazy_pixels() {
        struct NeverDecode;
        impl crate::LazyBgra for NeverDecode {
            fn bgra(&self) -> Option<Vec<u8>> {
                panic!("the owner walk must not allocate pixels");
            }
        }
        let mask = std::sync::Arc::new(
            crate::RenderImage::from_lazy_bgra(4096, 4096, std::sync::Arc::new(NeverDecode))
                .unwrap(),
        );
        let mut content = Scene::default();
        content.groups.push(PaintGroup {
            mask: Some(mask.clone()),
            ..Default::default()
        });
        let layer = crate::LayerHandle::new(SceneLayer::finish(content, Vec::new(), Vec::new()));
        let mut root = Scene::default();
        root.insert_layer(
            &layer,
            crate::Bounds::new(
                crate::point(0.0.into(), 0.0.into()),
                crate::size(10.0.into(), 10.0.into()),
            ),
        );
        drop(layer);
        let mut blocks = 0;
        root.heap_report(&mut |path, _, _| {
            if path == "display-list/mask-images" {
                blocks += 1;
            }
        });
        assert_eq!(
            blocks, 1,
            "only the image identity, no resident lazy pixels"
        );
    }
}
