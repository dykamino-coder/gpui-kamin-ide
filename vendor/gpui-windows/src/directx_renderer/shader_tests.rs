//! Компиляция всех debug-шейдеров без пути к исходному worktree в байткоде.

use super::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};

#[test]
fn debug_shader_bytecode_is_independent_of_the_source_directory() {
    let directory = env!("CARGO_MANIFEST_DIR");
    let modules = [
        ShaderModule::Quad,
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
