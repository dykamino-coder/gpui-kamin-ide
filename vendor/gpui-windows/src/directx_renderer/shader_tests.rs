//! Компиляция всех debug-шейдеров без пути к исходному worktree в байткоде.

use super::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};

/// Only the analytic-gradient program binds the stop buffers: the plain quad
/// and path programs must not carry the stop loop or its t5/t6 views.
#[test]
fn only_the_gradient_program_binds_the_stop_buffers() {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DReflect;
    use windows::Win32::Graphics::Direct3D::{D3D_SIT_CBUFFER, D3D_SIT_SAMPLER};
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_SHADER_DESC, D3D11_SHADER_INPUT_BIND_DESC, ID3D11ShaderReflection,
    };
    use windows::core::Interface;
    let bind_points = |module| {
        let shader = RawShaderBytes::new(module, ShaderTarget::Fragment).unwrap();
        let bytes = shader.as_bytes();
        let mut raw = std::ptr::null_mut();
        unsafe {
            D3DReflect(bytes.as_ptr().cast(), bytes.len(), &ID3D11ShaderReflection::IID, &mut raw)
                .unwrap();
            let reflection = ID3D11ShaderReflection::from_raw(raw);
            let mut desc = D3D11_SHADER_DESC::default();
            reflection.GetDesc(&mut desc).unwrap();
            // Shader resource views (t registers) only, not b/s registers.
            (0..desc.BoundResources)
                .filter_map(|index| {
                    let mut bind = D3D11_SHADER_INPUT_BIND_DESC::default();
                    reflection.GetResourceBindingDesc(index, &mut bind).unwrap();
                    (bind.Type != D3D_SIT_CBUFFER && bind.Type != D3D_SIT_SAMPLER)
                        .then_some(bind.BindPoint)
                })
                .collect::<Vec<_>>()
        }
    };
    let gradient = bind_points(ShaderModule::QuadGradient);
    assert!(gradient.contains(&5) && gradient.contains(&6), "{gradient:?}");
    for module in [ShaderModule::Quad, ShaderModule::PathSprite] {
        let points = bind_points(module);
        assert!(!points.contains(&5) && !points.contains(&6), "{module:?}: {points:?}");
    }
}

#[test]
fn debug_shader_bytecode_is_independent_of_the_source_directory() {
    let directory = env!("CARGO_MANIFEST_DIR");
    let modules = [
        ShaderModule::Quad,
        ShaderModule::QuadGradient,
        ShaderModule::Shadow,
        ShaderModule::Underline,
        ShaderModule::PathRasterization,
        ShaderModule::PathSprite,
        ShaderModule::MonochromeSprite,
        ShaderModule::SubpixelSprite,
        ShaderModule::PolychromeSprite,
        ShaderModule::EmojiRasterization,
        ShaderModule::Blur,
        ShaderModule::Clear,
        ShaderModule::GroupTiles,
    ];
    for module in modules {
        for target in [ShaderTarget::Vertex, ShaderTarget::Fragment] {
            let shader = RawShaderBytes::new(module, target).unwrap();
            let bytes = shader.as_bytes();
            assert!(!bytes.is_empty(), "{module:?}/{target:?}");
            for path in [directory.replace('\\', "/"), directory.replace('/', "\\")] {
                let wide: Vec<_> = path.encode_utf16().flat_map(u16::to_le_bytes).collect();
                assert!(
                    !bytes
                        .windows(path.len())
                        .any(|part| part == path.as_bytes())
                        && !bytes.windows(wide.len()).any(|part| part == wide),
                    "{module:?}/{target:?} retains its build directory"
                );
            }
        }
    }
}
