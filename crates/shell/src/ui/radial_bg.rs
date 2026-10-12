//! Испечённое радиальное свечение под логотипом welcome-экрана.
//!
//! Радиального фона окна здесь больше нет (INC-2026-0001): два свечения
//! с пиком 4–5 % рисовались стеком из ~22 почти прозрачных квадов размером
//! с окно, и без видеокарты одни они стоили около трети кадра. Небольшой
//! спрайт под логотипом печётся один раз и рисуется одной картинкой.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Image, ImageFormat};
use kamin_theme::Color;

/// Кэш испечённых свечений: ключ — (размер, цвет, пик, край). Шторка
/// переключения чата зовёт бейк КАЖДЫЙ кадр, а это генерация и PNG-кодирование
/// 150×150 (ревью ц.23).
/// Ключ — (размер, цвет, пик, край).
type GlowCache = std::sync::Mutex<HashMap<(u32, u32, u32, u32), Arc<Image>>>;
static GLOW_CACHE: std::sync::LazyLock<GlowCache> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// Радиальное свечение с явной точкой затухания `edge`: у свечения под лого
/// градиент гаснет на 68 % радиуса (`.logoWrap::before { …, transparent 68% }`,
/// ревью ц.15).
pub fn bake_glow_edge(size: u32, color: Color, peak_alpha: f32, edge: f32) -> Arc<Image> {
    let key = (
        size,
        color.r.to_bits() ^ color.g.to_bits().rotate_left(8) ^ color.b.to_bits().rotate_left(16),
        peak_alpha.to_bits(),
        edge.to_bits(),
    );
    if let Some(hit) = GLOW_CACHE.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let baked = bake_glow_edge_uncached(size, color, peak_alpha, edge);
    GLOW_CACHE.lock().unwrap().insert(key, baked.clone());
    baked
}

fn bake_glow_edge_uncached(size: u32, color: Color, peak_alpha: f32, edge: f32) -> Arc<Image> {
    let c = size as f32 / 2.0;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let r = (color.r * 255.0) as u8;
    let g = (color.g * 255.0) as u8;
    let b = (color.b * 255.0) as u8;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - c) / c;
            let dy = (y as f32 - c) / c;
            let d = (dx * dx + dy * dy).sqrt();
            // Плавный (smoothstep) спад до нуля к d = edge
            let t = (1.0 - (d / edge)).clamp(0.0, 1.0);
            let a = peak_alpha * t * t * (3.0 - 2.0 * t);
            let i = ((y * size + x) * 4) as usize;
            // ПРЕМУЛЬТИПЛИЦИРОВАННО: PNG-декод gpui (`frames_for_image`)
            // альфу не домножает, а шейдер блендит как premultiplied. С
            // прямой альфой rgb оставались полными при нулевой альфе — вся
            // квадратная область спрайта светилась ровным цветом («серый
            // квадрат вместо свечения» в лоадере чата).
            pixels[i] = (r as f32 * a) as u8;
            pixels[i + 1] = (g as f32 * a) as u8;
            pixels[i + 2] = (b as f32 * a) as u8;
            pixels[i + 3] = (a * 255.0) as u8;
        }
    }
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&pixels).expect("png data");
    }
    Arc::new(Image::from_bytes(ImageFormat::Png, png_bytes))
}
