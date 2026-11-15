//! Встроенные исходники debug-шейдеров и их общего HLSL include.

use super::shader_resources::{ShaderModule, ShaderTarget};
use anyhow::Result;
use std::ffi::c_void;
use windows::{
    Win32::{
        Foundation::E_INVALIDARG,
        Graphics::Direct3D::{
            D3D_INCLUDE_TYPE,
            Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompile},
            ID3DBlob, ID3DInclude, ID3DInclude_Impl,
        },
    },
    core::{PCSTR, s},
};

const ALPHA_CORRECTION: &[u8] = include_bytes!("../alpha_correction.hlsl");

struct EmbeddedIncludes;

#[allow(non_snake_case)]
impl ID3DInclude_Impl for EmbeddedIncludes {
    fn Open(
        &self,
        _include_type: D3D_INCLUDE_TYPE,
        filename: &PCSTR,
        _parent: *const c_void,
        data: *mut *mut c_void,
        size: *mut u32,
    ) -> windows::core::Result<()> {
        // SAFETY: D3DCompile supplies the NUL-terminated include name and valid out-pointers.
        // The static bytes remain alive through every Open/Close and the synchronous compile.
        unsafe {
            if filename.as_bytes() != b"alpha_correction.hlsl" {
                return Err(E_INVALIDARG.into());
            }
            *data = ALPHA_CORRECTION.as_ptr().cast_mut().cast();
            *size = ALPHA_CORRECTION.len() as u32;
        }
        Ok(())
    }

    fn Close(&self, _data: *const c_void) -> windows::core::Result<()> {
        Ok(())
    }
}

pub(super) fn build_shader_blob(entry: ShaderModule, target: ShaderTarget) -> Result<ID3DBlob> {
    // Rooted virtual names stop D3DCompiler from prefixing the worktree in debug info.
    let (source, name): (&[u8], PCSTR) = if matches!(entry, ShaderModule::EmojiRasterization) {
        (
            include_bytes!("../color_text_raster.hlsl"),
            s!("/gpui/color_text_raster.hlsl"),
        )
    } else {
        (include_bytes!("../shaders.hlsl"), s!("/gpui/shaders.hlsl"))
    };
    let entry = format!(
        "{}_{}\0",
        entry.as_str(),
        match target {
            ShaderTarget::Vertex => "vertex",
            ShaderTarget::Fragment => "fragment",
        }
    );
    let target = match target {
        ShaderTarget::Vertex => s!("vs_4_1"),
        ShaderTarget::Fragment => s!("ps_4_1"),
    };
    let includes = EmbeddedIncludes;
    let include_handler = ID3DInclude::new(&includes);
    let mut compile_blob = None;
    let mut error_blob = None;
    // SAFETY: all source/entry/include storage outlives this synchronous call; the compiler
    // owns the returned COM blobs. Only logical filenames enter the debug information.
    let ret = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            name,
            None,
            &*include_handler,
            PCSTR::from_raw(entry.as_ptr()),
            target,
            D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION,
            0,
            &mut compile_blob,
            Some(&mut error_blob),
        )
    };
    if ret.is_err() {
        let Some(error_blob) = error_blob else {
            return Err(anyhow::anyhow!("{ret:?}"));
        };
        // SAFETY: a compiler error blob contains NUL-terminated diagnostic text and is live.
        let error_string = unsafe {
            std::ffi::CStr::from_ptr(error_blob.GetBufferPointer() as *const i8).to_string_lossy()
        };
        log::error!("Shader compile error: {}", error_string);
        return Err(anyhow::anyhow!("Compile error: {}", error_string));
    }
    Ok(compile_blob.unwrap())
}
