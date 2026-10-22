//! CPU-граница SVG: шрифты внедрены, размер/плотность/поля заданы явно.
//! Растры принадлежат документу; глобального кэша по короткому хэшу здесь нет.

mod pixels;
use super::types::{AlphaMode, ColourSpace, CpuRaster, PixelFormat};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};
use std::sync::Arc;

/// Все внешние байты получены владельцем документа; бэкенд ничего не читает.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExternalImage {
    pub(crate) href: String,
    pub(crate) revision: crate::resources::loader::Revision,
    pub(crate) bytes: Option<Arc<Vec<u8>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RasterRequest {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) density: u32,
    pub(crate) pad: u32,
}
impl RasterRequest {
    pub(crate) fn new(w: f32, h: f32, density: f32, pad: f32) -> Self {
        Self {
            width: (w * density).round().max(1.0) as u32,
            height: (h * density).round().max(1.0) as u32,
            density: density.to_bits(),
            pad: (pad.max(0.0) * density).round() as u32,
        }
    }
}

pub(crate) fn rasterize(
    markup: &str,
    request: RasterRequest,
    fonts: Arc<usvg::fontdb::Database>,
    images: &[ExternalImage],
    allow_data: bool,
) -> Option<Arc<CpuRaster>> {
    let options = usvg::Options {
        fontdb: fonts,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(move |mime, bytes, opts| {
                allow_data
                    .then(|| usvg::ImageHrefResolver::default_data_resolver()(mime, bytes, opts))
                    .flatten()
            }),
            resolve_string: Box::new(move |href, opts| {
                let bytes = images
                    .iter()
                    .find(|image| {
                        // URI file: и абсолютный путь обозначают один ресурс;
                        // декодер не должен повторно читать его по другому имени.
                        let resource = |s: &str| {
                            crate::resources::loader::ResourceUrl::resolve(s, None)
                                .0
                                .base_url()
                        };
                        image.href == href
                            || resource(&image.href).is_some_and(|u| Some(u) == resource(href))
                    })?
                    .bytes
                    .clone()?;
                usvg::ImageHrefResolver::default_data_resolver()("text/plain", bytes, opts)
            }),
        },
        ..usvg::Options::default()
    };
    let (w, h) = (request.width, request.height);
    if w > 4096 || h > 4096 || request.pad > 4096 {
        return None;
    }
    let (pw, ph) = (w.checked_add(request.pad)?, h.checked_add(request.pad)?);
    let mut pixmap = Pixmap::new(pw, ph)?;
    // SVG 2 §8.6: a zero viewBox disables rendering without making the
    // image a failed resource. Preserve its transparent concrete viewport.
    if super::vector::degenerate_viewbox(markup) {
        return (request.pad == 0)
            .then(|| super::pixels::bgra(pw, ph, pixmap.take()))
            .flatten();
    }
    let tree = usvg::Tree::from_str(markup, &options).ok()?;
    let size = tree.size();
    resvg::render(
        &tree,
        Transform::from_scale(w as f32 / size.width(), h as f32 / size.height()),
        &mut pixmap.as_mut(),
    );
    if request.pad > 0
        && !(0..ph as usize).any(|y| {
            (0..pw as usize).any(|x| {
                (x >= w as usize || y >= h as usize)
                    && pixmap.data()[(y * pw as usize + x) * 4 + 3] != 0
            })
        })
    {
        return None;
    }
    let solid = pixels::solid_color(&tree);
    let mut bytes = pixmap.take();
    // Однотонная краска сохраняет свой точный цвет при любом покрытии.
    for p in bytes.chunks_exact_mut(4) {
        pixels::swap_and_unpremultiply(p);
        if let Some(c) = solid {
            p[..3].copy_from_slice(&[c.blue, c.green, c.red]);
        }
    }
    Some(Arc::new(CpuRaster {
        width: pw,
        height: ph,
        stride: pw as usize * 4,
        format: PixelFormat::Bgra8,
        alpha: AlphaMode::Straight,
        colour_space: ColourSpace::Srgb,
        bytes: bytes.into(),
    }))
}
