//! A program belongs to its pipeline generation and is created by its first draw.

use super::*;
use std::cell::{OnceCell, RefCell};

pub(super) struct Shader<S> {
    value: OnceCell<S>,
    pending: RefCell<Option<RawShaderBytes<'static>>>,
    create: fn(&ID3D11Device, &[u8]) -> Result<S>,
    profile: gpui::StartupProfile,
    interval: &'static str,
}

impl<S> Shader<S> {
    fn new(
        module: ShaderModule,
        target: ShaderTarget,
        create: fn(&ID3D11Device, &[u8]) -> Result<S>,
        profile: &gpui::StartupProfile,
        interval: &'static str,
    ) -> Result<(Self, String)> {
        let raw = RawShaderBytes::new(module, target)?;
        let identity = shader_identity::executable(raw.as_bytes())?;
        Ok((
            Self {
                value: OnceCell::new(),
                pending: RefCell::new(Some(raw)),
                create,
                profile: profile.clone(),
                interval,
            },
            identity,
        ))
    }

    pub(super) fn get(&self, context: &ID3D11DeviceContext) -> Result<&S> {
        if self.value.get().is_none() {
            // Look up the device only at creation; subsequent draws avoid COM lookups.
            let device = unsafe { context.GetDevice() }?;
            let mut pending = self.pending.borrow_mut();
            let raw = pending.as_ref().expect("uncreated program owns its bytes");
            let _span = self.profile.span(self.interval);
            let shader = (self.create)(&device, raw.as_bytes())?;
            let _ = self.value.set(shader);
            // Debug DXBC is compiled once too; release its blob after successful creation.
            pending.take();
        }
        Ok(self.value.get().expect("initialized above"))
    }

    pub(super) fn is_created(&self) -> bool {
        self.value.get().is_some()
    }
}

pub(super) fn vertex(
    module: ShaderModule,
    profile: &gpui::StartupProfile,
) -> Result<(Shader<ID3D11VertexShader>, String)> {
    Shader::new(
        module,
        ShaderTarget::Vertex,
        create_vertex_shader,
        profile,
        "renderer.vertex_shader_create",
    )
}

pub(super) fn fragment(
    module: ShaderModule,
    profile: &gpui::StartupProfile,
) -> Result<(Shader<ID3D11PixelShader>, String)> {
    Shader::new(
        module,
        ShaderTarget::Fragment,
        create_fragment_shader,
        profile,
        "renderer.pixel_shader_create",
    )
}
