use crate::{AssetSource, DevicePixels, IsZero, Result, SharedString, Size};
use resvg::tiny_skia::Pixmap;
use std::{
    hash::Hash,
    sync::{Arc, LazyLock},
};

/// When rendering SVGs, we render them at twice the size to get a higher-quality result.
pub const SMOOTH_SVG_SCALE_FACTOR: f32 = 2.;

#[derive(Clone, PartialEq, Hash, Eq)]
pub(crate) struct RenderSvgParams {
    pub(crate) path: SharedString,
    pub(crate) size: Size<DevicePixels>,
}

#[derive(Clone)]
pub struct SvgRenderer {
    asset_source: Arc<dyn AssetSource>,
    usvg_options: Arc<usvg::Options<'static>>,
}

pub enum SvgSize {
    Size(Size<DevicePixels>),
    ScaleFactor(f32),
}

/// KaminIDE patch: разметка SVG → готовое цветное изображение.
///
/// Штатных путей два, и оба не годятся для рисунка из документа: элемент
/// `svg()` превращает картинку в одноцветную маску (это путь иконок, цвета
/// теряются), а `img()` принимает только источник-ресурс, тогда как разметка
/// приходит строкой из чужого HTML. Поэтому — растеризация из строки, теми же
/// resvg/usvg, что уже стоят в GPUI: тащить их вторым комплектом в свой крейт
/// значило бы держать две копии одного растеризатора.
///
/// `None`, если разметка не разбирается: вызывающий покажет запасной текст.
pub fn svg_markup_to_image(
    markup: &str,
    width: f32,
    height: f32,
    density: f32,
) -> Option<Arc<crate::RenderImage>> {
    use image::{Frame, ImageBuffer};

    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(markup, &opts).ok()?;
    let pw = (width * density).round().max(1.0) as u32;
    let ph = (height * density).round().max(1.0) as u32;
    // Потолок: рисунок в чате не бывает больше половины экрана, а промах в
    // атрибутах размера иначе съел бы память гигабайтами.
    if pw > 4096 || ph > 4096 {
        return None;
    }
    let mut pixmap = Pixmap::new(pw, ph)?;
    let size = tree.size();
    let scale = (pw as f32 / size.width()).min(ph as f32 / size.height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let mut buffer = ImageBuffer::from_raw(pw, ph, pixmap.take())?;
    for pixel in buffer.chunks_exact_mut(4) {
        crate::swap_rgba_pa_to_bgra(pixel);
    }
    let mut image = crate::RenderImage::new(smallvec::SmallVec::from_elem(Frame::new(buffer), 1));
    image.scale_factor = density;
    Some(Arc::new(image))
}

impl SvgRenderer {
    pub fn new(asset_source: Arc<dyn AssetSource>) -> Self {
        static FONT_DB: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        });
        let default_font_resolver = usvg::FontResolver::default_font_selector();
        let font_resolver = Box::new(
            move |font: &usvg::Font, db: &mut Arc<usvg::fontdb::Database>| {
                if db.is_empty() {
                    *db = FONT_DB.clone();
                }
                default_font_resolver(font, db)
            },
        );
        let options = usvg::Options {
            font_resolver: usvg::FontResolver {
                select_font: font_resolver,
                select_fallback: usvg::FontResolver::default_fallback_selector(),
            },
            ..Default::default()
        };
        Self {
            asset_source,
            usvg_options: Arc::new(options),
        }
    }

    pub(crate) fn render(
        &self,
        params: &RenderSvgParams,
    ) -> Result<Option<(Size<DevicePixels>, Vec<u8>)>> {
        anyhow::ensure!(!params.size.is_zero(), "can't render at a zero size");

        // Load the tree.
        let Some(bytes) = self.asset_source.load(&params.path)? else {
            return Ok(None);
        };

        let pixmap = self.render_pixmap(&bytes, SvgSize::Size(params.size))?;

        // Convert the pixmap's pixels into an alpha mask.
        let size = Size::new(
            DevicePixels(pixmap.width() as i32),
            DevicePixels(pixmap.height() as i32),
        );
        let alpha_mask = pixmap
            .pixels()
            .iter()
            .map(|p| p.alpha())
            .collect::<Vec<_>>();
        Ok(Some((size, alpha_mask)))
    }

    pub fn render_pixmap(&self, bytes: &[u8], size: SvgSize) -> Result<Pixmap, usvg::Error> {
        let tree = usvg::Tree::from_data(bytes, &self.usvg_options)?;
        let svg_size = tree.size();
        let scale = match size {
            SvgSize::Size(size) => size.width.0 as f32 / svg_size.width(),
            SvgSize::ScaleFactor(scale) => scale,
        };

        // Render the SVG to a pixmap with the specified width and height.
        let mut pixmap = resvg::tiny_skia::Pixmap::new(
            (svg_size.width() * scale) as u32,
            (svg_size.height() * scale) as u32,
        )
        .ok_or(usvg::Error::InvalidSize)?;

        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Ok(pixmap)
    }
}
