//! KaminIDE patch: исполняемые DXBC chunks отделены от нестабильных debug paths.

use super::*;
use sha2::{Digest, Sha256};

pub(super) fn executable(bytes: &[u8]) -> Result<String> {
    anyhow::ensure!(bytes.starts_with(b"DXBC") && bytes.len() >= 32, "Invalid DXBC container");
    let word = |offset: usize| -> Result<usize> {
        Ok(u32::from_le_bytes(bytes.get(offset..offset + 4).context("Truncated DXBC word")?.try_into()?) as usize)
    };
    let mut digest = Sha256::new();
    let mut found = false;
    for index in 0..word(28)? {
        let offset = word(32 + index * 4)?;
        let tag = bytes.get(offset..offset + 4).context("Truncated DXBC chunk")?;
        if tag == b"SHDR" || tag == b"SHEX" {
            let size = word(offset + 4)?;
            digest.update(bytes.get(offset..offset + 8 + size).context("Truncated shader executable")?);
            found = true;
        }
    }
    anyhow::ensure!(found, "DXBC contains no executable chunk");
    Ok(format!("{:x}", digest.finalize()))
}

impl<T> PipelineState<T> {
    pub(super) fn new(
        device: &ID3D11Device,
        profile: &gpui::StartupProfile,
        label: &'static str,
        shader_module: ShaderModule,
        buffer_size: usize,
        blend_state: ID3D11BlendState,
    ) -> Result<Self> {
        let (vertex, vertex_identity) = deferred_shader::vertex(shader_module, profile)?;
        let (fragment, fragment_identity) = deferred_shader::fragment(shader_module, profile)?;
        let shader_executables = [vertex_identity, fragment_identity];
        let buffer = create_buffer(device, std::mem::size_of::<T>(), buffer_size)?;
        let view = create_buffer_view(device, &buffer)?;

        Ok(PipelineState {
            label,
            gradients: None,
            shader_executables,
            vertex,
            fragment,
            buffer,
            buffer_size,
            view,
            blend_state,
            _marker: std::marker::PhantomData,
        })
    }

}
