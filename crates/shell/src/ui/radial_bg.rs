//! Радиальный фон приложения (AppLayout .appWrapper, plan/24 §2).
//! В gpui 0.2.2 radial-градиентов нет → бейк: каждый эллипс печётся в
//! RenderImage (гамма-кодированный premultiplied BGRA) и рисуется прямым
//! `paint_image` поверх bg-sidebar.
//! CSS: radial-gradient(ellipse WxH at X% Y%, color A% → transparent 60%) —
//! эллипс ФИКСИРОВАННОГО размера, центр на процентной позиции вьюпорта.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, Bounds, Image, ImageFormat, Pixels, div, px};
use kamin_theme::{Color, Palette};

/// Кэш испечённых свечений: ключ — (размер, цвет, пик, край). Шторка
/// переключения чата зовёт бейк КАЖДЫЙ кадр, а это генерация и PNG-кодирование
/// 150×150 (ревью ц.23).
/// Ключ — (размер, цвет, пик, край).
type GlowCache = std::sync::Mutex<HashMap<(u32, u32, u32, u32), Arc<Image>>>;
static GLOW_CACHE: std::sync::LazyLock<GlowCache> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// Радиальное свечение с явной точкой затухания `edge`: у фона окна градиент
/// гаснет к самому краю (0.98), а у свечения под лого — на 68 % радиуса
/// (`.logoWrap::before { …, transparent 68% }`, ревью ц.15).
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

/// Смесь каналов к собственной яркости (Rec.709): k=0 — без изменений,
/// k=1 — полностью серый той же светлоты.
fn desat(c: Color, k: f32) -> Color {
    let l = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
    Color {
        r: c.r + (l - c.r) * k,
        g: c.g + (l - c.g) * k,
        b: c.b + (l - c.b) * k,
        a: c.a,
    }
}

/// Одно свечение фона: цвет + пиковая альфа + РАДИУСЫ эллипса (как в CSS:
/// `radial-gradient(ellipse 1200px 600px …)` — числа радиусы, не диаметры).
#[derive(Clone, Copy)]
struct Glow {
    color: Color,
    peak: f32,
    rx: f32,
    ry: f32,
}

pub struct RadialBg {
    purple: Glow,  // ellipse 1200×600 @ 20% 10%, accent-purple 8%
    primary: Glow, // ellipse 800×500 @ 90% 90%, accent-primary 6%
}

/// Стек вложенных «стадион»-квадов, аппроксимирующий эллиптическое свечение
/// БЕЗ ТЕКСТУР. Спрайты фона в атласе CEF-сборка ЛОМАЕТ ДВАЖДЫ: сначала
/// теряла (пустой фон), а после обхода — перезаписывала регион атласа чужой
/// текстурой (на фоне всплыл растянутый кадр КОНСОЛИ — скрин юзера; тот же
/// класс, что убил спрайт свечения лоадера → вектор). paint_quad не трогает
/// атлас вовсе.
///
/// КВАНТОВАНИЕ (грабли «градиент пропал»): рендер режет альфу в 8 бит, и
/// дельта кольца меньше 1/255 округляется В НОЛЬ — 64 кольца по 0.1% дали
/// пустой фон, а 24 по 0.33% округлялись ВВЕРХ до 1/255 и пересвечивали.
/// Поэтому колец ровно столько, сколько КВАНТОВ у пика (peak·255), каждое —
/// ровно один квант, а РАЗМЕРЫ колец стоят на изо-контурах smoothstep-профиля
/// (обратный smoothstep): спад плавный у пика и у края, ступень = 1 rgb-юнит,
/// глаже на 8-битном выходе не бывает без дизеринга.
fn glow_quads(window: &mut gpui::Window, g: Glow, cx0: f32, cy0: f32) {
    // Два ПОЛУФАЗНЫХ стека по половине пика: границы колец двух стеков не
    // совпадают, эффективная пространственная ступень — половина кванта.
    // Так браузерные градиенты выглядят «без кругов» (дизеринг у нас
    // недоступен, интерливинг фаз — ближайший аналог).
    let half = Glow {
        peak: g.peak / 2.0,
        ..g
    };
    glow_pass(window, half, cx0, cy0, 0.0);
    glow_pass(window, half, cx0, cy0, 0.5);
}

fn glow_pass(window: &mut gpui::Window, g: Glow, cx0: f32, cy0: f32, phase: f32) {
    let base = gpui::Rgba {
        r: g.color.r,
        g: g.color.g,
        b: g.color.b,
        a: 0.0,
    };
    let n = ((g.peak * 255.0).round() as usize).max(4);
    let delta = g.peak / n as f32;
    // Обратный smoothstep: x такой, что x²(3−2x) = y (закрытая форма).
    let inv_smoothstep =
        |y: f32| -> f32 { 0.5 - ((1.0 - 2.0 * y.clamp(0.0, 1.0)).asin() / 3.0).sin() };
    for k in 1..=n {
        // k-е кольцо накрывает зону, где кумулятивная альфа ≥ k квантов:
        // x — доля пути от края к центру, t — параметр размера (0.6 = край
        // CSS-градиента, дальше прозрачен). `phase` сдвигает границы колец
        // этого стека на долю ступени (полуфазный интерливинг выше).
        let x = inv_smoothstep((k as f32 - phase).max(0.001) / n as f32);
        let t = 0.6 * (1.0 - x).max(0.02);
        let (w, h) = (g.rx * t * 2.0, g.ry * t * 2.0);
        let mut c = base;
        c.a = delta;
        let mut q = gpui::fill(
            gpui::Bounds {
                origin: gpui::point(px(cx0 - w / 2.0), px(cy0 - h / 2.0)),
                size: gpui::size(px(w), px(h)),
            },
            c,
        );
        // Полное скругление короткой стороны: «стадион» — ближайшая к
        // эллипсу форма, которую умеет paint_quad; на мягких альфах
        // разница с истинным эллипсом не читается.
        q.corner_radii = gpui::Corners::all(px(h.min(w) / 2.0));
        window.paint_quad(q);
    }
}

impl RadialBg {
    pub fn bake(p: &Palette) -> Self {
        Self {
            // Пики 5%/4% (CSS-оригинал 8%/6%) — финальный выбор юзера;
            // оттенок сверен с Tauri-скрином: та же лаванда #cba6f7.
            purple: Glow {
                color: p.accent_purple,
                peak: 0.05,
                rx: 1200.0,
                ry: 600.0,
            },
            primary: Glow {
                // Бирюза accent_primary в чистом виде читалась чужеродно
                // на дефолтной тёмной теме (низ окна) — насыщенность вдвое
                // вниз (смесь к собственной яркости): оттенок приближается
                // к общему цвету фона, свечение остаётся.
                color: desat(p.accent_primary, 0.5),
                peak: 0.04,
                rx: 800.0,
                ry: 500.0,
            },
        }
    }

    /// Два свечения; центры на 20%/10% и 90%/90% вьюпорта.
    pub fn layers(&self, viewport: Bounds<Pixels>) -> AnyElement {
        let vw = f32::from(viewport.size.width);
        let vh = f32::from(viewport.size.height);
        let purple = self.purple;
        let primary = self.primary;
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        glow_quads(window, purple, vw * 0.20, vh * 0.10);
                        glow_quads(window, primary, vw * 0.90, vh * 0.90);
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }
}
