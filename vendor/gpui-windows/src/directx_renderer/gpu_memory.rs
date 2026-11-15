//! KaminIDE patch: GPU memory the renderer holds, by owner, for the engine bench's memory
//! report (`Window::gpu_memory`). Sizes come from the texture descriptions: what the pixels
//! take, not what the driver reserved around them. Not walked: the swap chain's other
//! buffers, DirectWrite's glyph rasterization targets (they live for one glyph), buffers and
//! shaders.

use super::*;

/// Pixel storage of a texture: every mip level, array slice and MSAA sample.
pub(crate) fn texture_bytes(texture: &ID3D11Texture2D) -> u64 {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    described_bytes(&desc)
}

fn described_bytes(desc: &D3D11_TEXTURE2D_DESC) -> u64 {
    let texel = match desc.Format {
        DXGI_FORMAT_R8_UNORM | DXGI_FORMAT_A8_UNORM => 1,
        DXGI_FORMAT_R16G16B16A16_FLOAT => 8,
        _ => 4,
    };
    let (mut width, mut height) = (u64::from(desc.Width), u64::from(desc.Height));
    let mut texels = 0;
    // MipLevels 0 asks for the full chain.
    let levels = if desc.MipLevels == 0 { u32::MAX } else { desc.MipLevels };
    for _ in 0..levels {
        texels += width * height;
        if width <= 1 && height <= 1 {
            break;
        }
        (width, height) = ((width / 2).max(1), (height / 2).max(1));
    }
    texels * texel * u64::from(desc.ArraySize.max(1)) * u64::from(desc.SampleDesc.Count.max(1))
}

impl DirectXRenderer {
    /// One call per texture the renderer holds: owner path, bytes, bytes in use.
    pub(crate) fn gpu_memory(&self, sink: &mut dyn FnMut(&'static str, u64, u64)) {
        self.atlas.gpu_memory(sink);
        let mut whole = |path: &'static str, texture: &ID3D11Texture2D| {
            let bytes = texture_bytes(texture);
            sink(path, bytes, bytes);
        };
        if let Some(resources) = &self.resources {
            if let Some(target) = &resources.render_target {
                whole("gpu/target/frame", target);
            }
            whole("gpu/target/path", &resources.path_intermediate_texture);
            whole("gpu/target/path-msaa", &resources.path_intermediate_msaa_texture);
        }
        let blur = &self.blur;
        let single = [
            ("gpu/group/stage", &blur.stage),
            ("gpu/group/level-backdrop", &blur.level_backdrop),
            ("gpu/blur/copy", &blur.copy),
            ("gpu/blur/blend-copy", &blur.blend_copy),
            ("gpu/blur/gauss", &blur.gauss),
        ];
        for (path, texture) in single {
            if let Some(texture) = texture {
                whole(path, &texture.texture);
            }
        }
        let lists = [
            ("gpu/blur/down", blur.down.as_slice()),
            ("gpu/group/atlas", blur.groups.as_slice()),
            ("gpu/group/pool", blur.pool.idle()),
        ];
        for (path, textures) in lists {
            for texture in textures {
                whole(path, &texture.texture);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Not a glob: the parent's glob brings gpui's own `test` attribute into scope.
    use super::{
        D3D11_TEXTURE2D_DESC, DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8_UNORM,
        DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_SAMPLE_DESC, described_bytes,
    };

    fn desc(width: u32, height: u32, format: DXGI_FORMAT, mips: u32, samples: u32) -> D3D11_TEXTURE2D_DESC {
        D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: mips,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: samples, Quality: 0 },
            ..Default::default()
        }
    }

    #[test]
    fn bytes_follow_format_mips_and_samples() {
        assert_eq!(described_bytes(&desc(1024, 1024, DXGI_FORMAT_R8_UNORM, 1, 1)), 1 << 20);
        assert_eq!(described_bytes(&desc(800, 600, DXGI_FORMAT_B8G8R8A8_UNORM, 1, 4)), 800 * 600 * 16);
        assert_eq!(described_bytes(&desc(16, 16, DXGI_FORMAT_R16G16B16A16_FLOAT, 1, 1)), 16 * 16 * 8);
        assert_eq!(described_bytes(&desc(4, 2, DXGI_FORMAT_B8G8R8A8_UNORM, 0, 1)), (8 + 2 + 1) * 4);
    }
}
