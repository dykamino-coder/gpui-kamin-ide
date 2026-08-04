//! Пре-декод иконок дерева: SVG → `RenderImage` СИНХРОННО, со статическим
//! кэшем. Обходит асинхронный image_cache gpui целиком: его завершение декода
//! при точечной перерисовке было хрупким (пустые квадраты после переключений
//! сессий, краш-стек в зоне svg paint/decode — C++ exception 0xE06D7363), а
//! иконки — крошечные плоские SVG, которые дешевле разово растеризовать самим
//! и рисовать как готовую текстуру (`ImageSource::Render`).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use gpui::RenderImage;

/// Сторона растра: иконки дерева 16 лог.px, максимум hidpi ×2.
const SIZE: u32 = 32;

static CACHE: LazyLock<Mutex<HashMap<String, Option<Arc<RenderImage>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Растровая иконка по ключу; `load` зовётся только при промахе кэша.
/// `None` кэшируется тоже — битый SVG не перечитывается на каждый кадр.
/// `light` — применить светлый фильтр оригинала (`saturate(3.2)
/// brightness(0.7)`, TreeIcon.module.css) К ПИКСЕЛЯМ растра: переписывание
/// hex внутри SVG-исходника ломало `url(#id)`-ссылки и градиенты — иконки
/// светлой темы разваливались (скрин юзера).
pub fn raster_svg(
    key: &str,
    light: bool,
    load: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<Arc<RenderImage>> {
    if let Some(hit) = CACHE.lock().unwrap().get(key) {
        return hit.clone();
    }
    let out = load().and_then(|bytes| rasterize(&bytes, light));
    CACHE.lock().unwrap().insert(key.to_string(), out.clone());
    out
}

/// Прогрев атласа: растеризовать и НАРИСОВАТЬ (1px, под фоновым слоем) оба
/// варианта всех builtin-иконок в ПЕРВЫЕ кадры. Тайлы, аллоцированные рано,
/// CEF не затаптывает (одна тема работала стабильно); аллокации ПОЗЖЕ — при
/// смене темы — попадали в затоптанные регионы, и иконки показывали куски
/// CEF-кадра (скрин юзера). После прогрева смена темы ничего не аллоцирует.
pub fn warm_all(window: &mut gpui::Window) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    if FRAMES.fetch_add(1, Ordering::Relaxed) >= 3 {
        return;
    }
    for (asset, bytes) in crate::ui::icons::CAT_ICONS.iter() {
        for light in [false, true] {
            let key = format!("{asset}#{}", u8::from(light));
            if let Some(im) = raster_svg(&key, light, || Some(bytes.to_vec())) {
                let _ = window.paint_image(
                    gpui::Bounds {
                        origin: gpui::point(gpui::px(0.0), gpui::px(0.0)),
                        size: gpui::size(gpui::px(1.0), gpui::px(1.0)),
                    },
                    gpui::Corners::default(),
                    im,
                    0,
                    false,
                );
            }
        }
    }
}

fn rasterize(bytes: &[u8], light: bool) -> Option<Arc<RenderImage>> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)?;
    let size = tree.size();
    let scale = (SIZE as f32 / size.width()).min(SIZE as f32 / size.height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia хранит premultiplied RGBA; RenderImage ждёт premultiplied BGRA
    // (тот же контракт, что у CEF-кадров в web/frames.rs) — переставить каналы.
    let mut data = pixmap.take();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2);
        if light {
            // Цветовая матрица линейна — на premultiplied применима как есть
            // (M(c·a) = M(c)·a); альфа не трогается. Порядок каналов BGRA.
            let (r, g, b) = crate::icon_light::filter_rgb(px[2], px[1], px[0]);
            px[2] = r;
            px[1] = g;
            px[0] = b;
        }
    }
    let img = image::RgbaImage::from_raw(SIZE, SIZE, data)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(img)])))
}
