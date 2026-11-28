//! Immutable mask uploads retained by the current scene, without a count cap.
//!
//! Marking uses the complete scene, including groups outside the damage and
//! groups whose atlas images are retained. It also runs on kept frames.

use super::*;

pub(super) struct MaskUpload {
    pub(super) texture: ID3D11Texture2D,
    pub(super) view: [Option<ID3D11ShaderResourceView>; 1],
    epoch: u64,
}

impl MaskUpload {
    pub(super) fn new(
        texture: ID3D11Texture2D,
        view: [Option<ID3D11ShaderResourceView>; 1],
    ) -> Self {
        Self {
            texture,
            view,
            epoch: 0,
        }
    }
}

#[derive(Default)]
pub(super) struct MaskUploads {
    entries: HashMap<ImageId, MaskUpload>,
    epoch: u64,
}

impl MaskUploads {
    pub(super) fn begin_frame(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            for upload in self.entries.values_mut() {
                upload.epoch = u64::MAX;
            }
        }
    }

    pub(super) fn get(&mut self, id: ImageId) -> Option<[Option<ID3D11ShaderResourceView>; 1]> {
        let upload = self.entries.get_mut(&id)?;
        upload.epoch = self.epoch;
        Some(upload.view.clone())
    }

    pub(super) fn insert(
        &mut self,
        id: ImageId,
        mut upload: MaskUpload,
    ) -> [Option<ID3D11ShaderResourceView>; 1] {
        upload.epoch = self.epoch;
        let view = upload.view.clone();
        self.entries.insert(id, upload);
        view
    }

    #[cfg(test)]
    fn finish_frame(&mut self) {
        self.entries.retain(|_, upload| upload.epoch == self.epoch);
    }

    pub(super) fn retain_scene(
        &mut self,
        scene: &Scene,
        mut dropped: impl FnMut(&ID3D11Texture2D),
    ) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        let before = self.entries.len();
        self.begin_frame();
        // Groups are dependency-ordered in the root scene, including nested
        // groups. Their child scenes contain composite references to this list.
        for group in &scene.groups {
            if let Some(image) = &group.mask
                && let Some(upload) = self.entries.get_mut(&image.id)
            {
                upload.epoch = self.epoch;
            }
        }
        self.entries.retain(|_, upload| {
            if upload.epoch == self.epoch {
                true
            } else {
                dropped(&upload.texture);
                false
            }
        });
        before - self.entries.len()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn textures(&self) -> impl Iterator<Item = &ID3D11Texture2D> {
        self.entries.values().map(|upload| &upload.texture)
    }
}

#[cfg(test)]
mod tests {
    use super::{MaskUploads, create_mask_texture};
    use crate::DirectXDevices;
    use gpui::ImageId;

    #[test]
    fn more_than_sixty_four_current_masks_keep_the_same_uploads() {
        let device = DirectXDevices::new().unwrap();
        let mut cache = MaskUploads::default();
        let mut views = Vec::new();
        cache.begin_frame();
        for id in 0..96 {
            let upload = create_mask_texture(&device.device, 3, 2, &[127; 24]).unwrap();
            views.push(cache.insert(ImageId(id), upload));
        }
        cache.finish_frame();
        for _ in 0..3 {
            cache.begin_frame();
            for (id, view) in views.iter().enumerate() {
                assert_eq!(cache.get(ImageId(id)).as_ref(), Some(view));
            }
            cache.finish_frame();
            assert_eq!(cache.textures().count(), 96);
        }
        // A page using one old mask releases the other 95 immediately.
        cache.begin_frame();
        assert_eq!(cache.get(ImageId(17)).as_ref(), Some(&views[17]));
        cache.finish_frame();
        assert_eq!(cache.textures().count(), 1);
        cache.begin_frame();
        cache.finish_frame();
        assert!(cache.is_empty());
    }
}
