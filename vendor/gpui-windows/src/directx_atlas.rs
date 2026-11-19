use etagere::BucketedAtlasAllocator;
use parking_lot::Mutex;
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
    },
    Dxgi::Common::*,
};

use gpui::{
    AtlasBackend, AtlasKey, AtlasState, AtlasTextureId, AtlasTextureKind,
    AtlasTile, Bounds, DevicePixels, PlatformAtlas, Point, Size,
};

/// KaminIDE patch: the second field counts changes that alter the pixels of
/// tiles a scene may still reference (removed tiles, replaced or forgotten
/// external textures, device loss): a renderer keeping its last frame must
/// not reuse pixels drawn before such a change.
pub(crate) struct DirectXAtlas(
    Mutex<AtlasState<DirectXAtlasTextures>>,
    std::sync::atomic::AtomicU64,
    // KaminIDE patch: repainted rectangles of external textures.
    Mutex<external_damage::ExternalDamage>,
);

mod external_damage;
mod texture_slots;
use texture_slots::TextureSlots;

struct DirectXAtlasTextures {
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    monochrome_textures: TextureSlots<DirectXAtlasTexture>,
    polychrome_textures: TextureSlots<DirectXAtlasTexture>,
    subpixel_textures: TextureSlots<DirectXAtlasTexture>,
}

struct DirectXAtlasTexture {
    id: AtlasTextureId,
    bytes_per_pixel: u32,
    allocator: BucketedAtlasAllocator,
    texture: ID3D11Texture2D,
    view: [Option<ID3D11ShaderResourceView>; 1],
    live_atlas_keys: u32,
    /// KaminIDE patch: a texture of another producer (`register_external`),
    /// whose pixels change without the atlas knowing.
    external: bool,
}

impl DirectXAtlas {
    pub(crate) fn new(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Self {
        let state = Mutex::new(AtlasState::new(DirectXAtlasTextures {
            device: device.clone(),
            device_context: device_context.clone(),
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
            subpixel_textures: Default::default(),
        }));
        DirectXAtlas(
            state,
            std::sync::atomic::AtomicU64::new(0),
            Mutex::new(Default::default()),
        )
    }

    /// KaminIDE patch: see the second field of `DirectXAtlas`.
    pub(crate) fn epoch(&self) -> u64 {
        self.1.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn bump(&self) {
        self.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// KaminIDE patch: what the external textures sampled by `scene` changed
    /// since the last call (`external_damage.rs`): `Unchanged` without
    /// external textures or reported changes, `Unknown` when a change cannot
    /// be placed. Each planned frame calls it once.
    pub(crate) fn external_damage(&self, scene: &gpui::Scene) -> gpui::SceneDamage {
        let lock = self.0.lock();
        let list = &lock.backend.polychrome_textures;
        let is_external = |index: u32| {
            list.get(index)
                .is_some_and(|t| t.external)
        };
        self.2.lock().take(scene, is_external)
    }

    /// KaminIDE patch: the producer of external texture `id` repainted
    /// `rect` (texture pixels) of it. From the first call on, the texture's
    /// changes are placed by these rectangles; the producer must report
    /// every change.
    pub(crate) fn damage_external(&self, id: AtlasTextureId, rect: Bounds<DevicePixels>) {
        self.2.lock().mark(id.index, rect);
    }

    /// Returns the view backing `id`, or `None` once every tile in it has been
    /// removed. A scene can still reference such a texture when a cached view
    /// replays a paint from before the image was dropped, so callers must skip
    /// those sprites rather than assume the texture exists.
    // KaminIDE patch: None = слот освобождён (кадр «в полёте» пережил
    // unregister_external) — вызывающий пропускает отрисовку этой пачки.
    pub(crate) fn get_texture_view(
        &self,
        id: AtlasTextureId,
    ) -> Option<[Option<ID3D11ShaderResourceView>; 1]> {
        let lock = self.0.lock();
        let texture = lock.backend.texture(id)?;
        Some(texture.view.clone())
    }

    /// KaminIDE patch: положить ЧУЖУЮ текстуру D3D11 (готовый кадр CEF) в
    /// атлас отдельной записью и получить тайл на всю её площадь.
    ///
    /// Копий нет: спрайт будет выбирать пиксели прямо из текстуры Chromium,
    /// тем же шейдером, что и обычные картинки, — со скруглением углов.
    pub(crate) fn register_external(
        &self,
        texture: ID3D11Texture2D,
        size: Size<DevicePixels>,
    ) -> Option<AtlasTile> {
        let mut lock = self.0.lock();
        let view = unsafe {
            let mut view = None;
            lock.backend.device
                .CreateShaderResourceView(&texture, None, Some(&mut view))
                .ok()?;
            [view]
        };
        let list = &mut lock.backend.polychrome_textures;
        // Never reuse an id: an in-flight external scene may still reference it.
        let id = AtlasTextureId {
            index: list.next_id()?,
            kind: AtlasTextureKind::Polychrome,
        };
        let atlas_texture = DirectXAtlasTexture {
            id,
            bytes_per_pixel: 4,
            // Аллокатор чужой текстуре не нужен — тайл ровно один, на всю
            // площадь; заводим пустой, чтобы не плодить вариантов структуры.
            allocator: etagere::BucketedAtlasAllocator::new(device_size_to_etagere(size)),
            texture,
            view,
            live_atlas_keys: 0,
            external: true,
        };
        list.insert(id.index, atlas_texture);
        Some(AtlasTile {
            texture_id: id,
            tile_id: gpui::TileId(0),
            padding: 0,
            bounds: Bounds {
                origin: Point {
                    x: DevicePixels(0),
                    y: DevicePixels(0),
                },
                size,
            },
        })
    }

    /// KaminIDE patch: заменить содержимое записи, НЕ трогая её индекс.
    ///
    /// Так у вью на всё время жизни одна запись атласа: индексы не
    /// освобождаются и не переиспользуются, а значит кадр в полёте не может
    /// начать рисовать чужую текстуру (из-за этого приложение падало без
    /// паники на быстром ресайзе).
    pub(crate) fn update_external(
        &self,
        id: AtlasTextureId,
        texture: ID3D11Texture2D,
    ) -> bool {
        let mut lock = self.0.lock();
        let view = unsafe {
            let mut view = None;
            if lock
                .backend
                .device
                .CreateShaderResourceView(&texture, None, Some(&mut view))
                .is_err()
            {
                return false;
            }
            [view]
        };
        let Some(slot) = lock.backend.polychrome_textures.get_mut(id.index) else {
            return false;
        };
        slot.texture = texture;
        slot.view = view;
        drop(lock);
        self.2.lock().forget(id.index);
        self.bump();
        true
    }

    /// Drop an external texture without retaining a vacant slot or reusing its id.
    /// An older scene still naming the id will skip its missing texture.
    pub(crate) fn unregister_external(&self, id: AtlasTextureId) {
        let mut lock = self.0.lock();
        lock.backend.polychrome_textures.remove(id.index);
        drop(lock);
        self.2.lock().forget(id.index);
        self.bump();
    }

    pub(crate) fn handle_device_lost(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
    ) {
        let mut lock = self.0.lock();
        lock.clear(|textures| {
            textures.device = device.clone();
            textures.device_context = device_context.clone();
            textures.monochrome_textures = TextureSlots::default();
            textures.polychrome_textures = TextureSlots::default();
            textures.subpixel_textures = TextureSlots::default();
        });
        drop(lock);
        *self.2.lock() = Default::default();
        self.bump();
    }
}

impl DirectXAtlas {
    /// KaminIDE patch: every atlas page as (owner path, bytes, bytes of allocated tiles), see
    /// `DirectXRenderer::gpu_memory`. A registered external texture (a CEF frame) is a page of
    /// the polychrome list with no allocated tiles.
    pub(crate) fn gpu_memory(&self, sink: &mut dyn FnMut(&'static str, u64, u64)) {
        let lock = self.0.lock();
        let lists = [
            ("gpu/atlas/monochrome", &lock.backend.monochrome_textures),
            ("gpu/atlas/polychrome", &lock.backend.polychrome_textures),
            ("gpu/atlas/subpixel", &lock.backend.subpixel_textures),
        ];
        for (path, list) in lists {
            for texture in list.iter() {
                let used = texture.allocator.allocated_space().max(0) as u64;
                sink(
                    path,
                    crate::directx_renderer::texture_bytes(&texture.texture),
                    used * u64::from(texture.bytes_per_pixel),
                );
            }
        }
    }
}

impl PlatformAtlas for DirectXAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        self.0.lock().get_or_insert_with(key, build)
    }

    fn remove(&self, key: &AtlasKey) {
        self.0.lock().remove(key);
        self.bump();
    }
}

impl AtlasBackend for DirectXAtlasTextures {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> anyhow::Result<AtlasTile> {
        let tile = self
            .allocate(size, kind)
            .ok_or_else(|| anyhow::anyhow!("failed to allocate"))?;
        let texture = self
            .texture(tile.texture_id)
            .ok_or_else(|| anyhow::anyhow!("allocated tile refers to a missing texture"))?;
        texture.upload(&self.device_context, tile.bounds, bytes);
        Ok(tile)
    }

    fn remove(&mut self, tile: AtlasTile) {
        let id = tile.texture_id;

        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
        };

        if let Some(texture) = textures.get_mut(id.index) {
            texture.allocator.deallocate(tile.tile_id.into());
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                textures.remove(id.index);
            }
        }
    }
}

impl DirectXAtlasTextures {
    fn allocate(
        &mut self,
        size: Size<DevicePixels>,
        texture_kind: AtlasTextureKind,
    ) -> Option<AtlasTile> {
        {
            let textures = match texture_kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
                AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
            };

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return Some(tile);
            }
        }

        let texture = self.push_texture(size, texture_kind)?;
        texture.allocate(size)
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> Option<&mut DirectXAtlasTexture> {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        // Max texture size for DirectX. See:
        // https://learn.microsoft.com/en-us/windows/win32/direct3d11/overviews-direct3d-11-resources-limits
        const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(16384),
            height: DevicePixels(16384),
        };
        let size = min_size.min(&MAX_ATLAS_SIZE).max(&DEFAULT_ATLAS_SIZE);
        let pixel_format;
        let bind_flag;
        let bytes_per_pixel;
        match kind {
            AtlasTextureKind::Monochrome => {
                pixel_format = DXGI_FORMAT_R8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 1;
            }
            AtlasTextureKind::Polychrome => {
                pixel_format = DXGI_FORMAT_B8G8R8A8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 4;
            }
            AtlasTextureKind::Subpixel => {
                pixel_format = DXGI_FORMAT_R8G8B8A8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 4;
            }
        }
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: size.width.0 as u32,
            Height: size.height.0 as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: pixel_format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind_flag.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe {
            // This only returns None if the device is lost, which we will recreate later.
            // So it's ok to return None here.
            self.device
                .CreateTexture2D(&texture_desc, None, Some(&mut texture))
                .ok()?;
        }
        let texture = texture.unwrap();

        let texture_list = match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
        };
        // Unique ids preserve stale-scene safety without retaining empty slots.
        let index = texture_list.next_id()?;
        let view = unsafe {
            let mut view = None;
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut view))
                .ok()?;
            [view]
        };
        let atlas_texture = DirectXAtlasTexture {
            id: AtlasTextureId {
                index,
                kind,
            },
            bytes_per_pixel,
            allocator: etagere::BucketedAtlasAllocator::new(device_size_to_etagere(size)),
            texture,
            view,
            live_atlas_keys: 0,
            external: false,
        };
        Some(texture_list.insert(index, atlas_texture))
    }

    // KaminIDE patch: Option вместо unwrap-паники. Слот может быть None:
    // текстура вью снята unregister_external, а кадр с её спрайтом ещё в
    // полёте (CEF-насос реентерабелен) — тогда спрайт просто не рисуем.
    fn texture(&self, id: AtlasTextureId) -> Option<&DirectXAtlasTexture> {
        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
            AtlasTextureKind::Subpixel => &self.subpixel_textures,
        };
        textures.get(id.index)
    }
}

impl DirectXAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let allocation = self.allocator.allocate(device_size_to_etagere(size))?;
        let tile = AtlasTile {
            texture_id: self.id,
            tile_id: allocation.id.into(),
            bounds: Bounds {
                origin: etagere_point_to_device(allocation.rectangle.min),
                size,
            },
            padding: 0,
        };
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn upload(
        &self,
        device_context: &ID3D11DeviceContext,
        bounds: Bounds<DevicePixels>,
        bytes: &[u8],
    ) {
        // `UpdateSubresource` reads `row_pitch * height` bytes from `bytes` based on the
        // `D3D11_BOX` below. If the caller hands us a slice shorter than that, the driver would
        // over-read past the end of the source buffer (potentially by multiple megabytes), so bail
        // out instead. This is a first-insert path rather than a per-frame one, so the check is
        // effectively free.
        let row_bytes = bounds.size.width.to_bytes(self.bytes_per_pixel as u8) as usize;
        let expected = row_bytes * bounds.size.height.0.max(0) as usize;
        if bytes.len() < expected {
            log::error!(
                "DirectXAtlasTexture::upload: source slice is {} bytes but the {}x{} region \
                 requires {} bytes; skipping upload to avoid a driver over-read",
                bytes.len(),
                bounds.size.width.0,
                bounds.size.height.0,
                expected,
            );
            return;
        }
        unsafe {
            device_context.UpdateSubresource(
                &self.texture,
                0,
                Some(&D3D11_BOX {
                    left: bounds.left().0 as u32,
                    top: bounds.top().0 as u32,
                    front: 0,
                    right: bounds.right().0 as u32,
                    bottom: bounds.bottom().0 as u32,
                    back: 1,
                }),
                bytes.as_ptr() as _,
                bounds.size.width.to_bytes(self.bytes_per_pixel as u8),
                0,
            );
        }
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys -= 1;
    }

    fn is_unreferenced(&mut self) -> bool {
        self.live_atlas_keys == 0
    }
}

fn device_size_to_etagere(size: Size<DevicePixels>) -> etagere::Size {
    etagere::Size::new(size.width.into(), size.height.into())
}

fn etagere_point_to_device(value: etagere::Point) -> Point<DevicePixels> {
    Point {
        x: DevicePixels::from(value.x),
        y: DevicePixels::from(value.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ImageId, RenderImageParams};
    use std::borrow::Cow;
    use windows::Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_WARP,
            Direct3D11::{D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice},
        },
    };

    fn create_atlas() -> Option<DirectXAtlas> {
        let mut device: Option<ID3D11Device> = None;
        let mut device_context: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut device_context),
            )
        }
        .ok()?;
        Some(DirectXAtlas::new(&device?, &device_context?))
    }

    fn make_image_key(image_id: usize) -> AtlasKey {
        AtlasKey::Image(RenderImageParams {
            image_id: ImageId(image_id),
            frame_index: 0,
        })
    }

    fn insert_tile(atlas: &DirectXAtlas, key: AtlasKey, size: Size<DevicePixels>) -> AtlasTile {
        atlas
            .get_or_insert_with(key, &mut || {
                let byte_count = (size.width.0 as usize) * (size.height.0 as usize) * 4;
                Ok(Some((size, Cow::Owned(vec![0u8; byte_count]))))
            })
            .expect("allocation should succeed")
            .expect("callback returns Some")
    }

    #[test]
    fn empty_pages_are_released_without_reusing_stale_scene_ids() {
        let atlas = create_atlas().expect("WARP device must be available");
        let size = Size {
            width: DevicePixels(700),
            height: DevicePixels(700),
        };
        let mut previous = None;
        for request in 0..64 {
            let key = make_image_key(request);
            let tile = insert_tile(&atlas, key.clone(), size);
            if let Some(old) = previous {
                assert_ne!(old, tile.texture_id);
                assert!(atlas.get_texture_view(old).is_none());
            }
            assert_eq!(atlas.0.lock().backend.polychrome_textures.iter().count(), 1);
            atlas.remove(&key);
            assert!(atlas.get_texture_view(tile.texture_id).is_none());
            assert_eq!(atlas.0.lock().backend.polychrome_textures.iter().count(), 0);
            previous = Some(tile.texture_id);
        }
    }

    #[test]
    fn test_remove_deallocates_tile_space_for_reuse() {
        let Some(atlas) = create_atlas() else {
            return;
        };

        let small = Size {
            width: DevicePixels(64),
            height: DevicePixels(64),
        };
        let big = Size {
            width: DevicePixels(700),
            height: DevicePixels(700),
        };

        let keeper_key = make_image_key(1);
        let big_key_a = make_image_key(2);
        let big_key_b = make_image_key(3);

        let keeper_tile = insert_tile(&atlas, keeper_key, small);
        let tile_a = insert_tile(&atlas, big_key_a.clone(), big);
        assert_eq!(keeper_tile.texture_id, tile_a.texture_id);

        atlas.remove(&big_key_a);

        let tile_b = insert_tile(&atlas, big_key_b, big);
        assert_eq!(tile_b.texture_id, keeper_tile.texture_id);
    }
}
