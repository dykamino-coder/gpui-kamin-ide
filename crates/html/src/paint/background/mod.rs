//! Фоновая картинка: `background-image: url(...)` со всей её механикой.
//!
//! Почему отдельным проходом, а не элементом `img`. Фон в CSS — это заливка:
//! она мостится плитками, смещается, масштабируется и обрезается по коробке,
//! причём независимо от содержимого элемента. Элемент-картинка так не умеет:
//! он рисует ровно одну копию и участвует в раскладке. Поэтому фон рисуется
//! канвасом, который знает свои границы во время отрисовки, и кладёт нужное
//! число копий сам.
//!
//! Образ декодируется один раз и лежит в кэше: разбор PNG на каждом кадре
//! стоил бы дороже всей остальной отрисовки документа.

use crate::style::computed::{BgRepeat, Computed, Tiling};
use gpui::{AnyElement, Bounds, IntoElement, Pixels, RenderImage, Styled, px};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
pub(crate) mod alpha_sampling;
mod float_geometry;
mod oriented_vector;
mod sampling;
use crate::paint::background::float_geometry::rrect_of;
pub use float_geometry::rounded_float;
mod mask_composite;
pub use mask_composite::{MaskLayer, compose_mask_layers};
mod conic;
mod sources;
pub use sources::{key, key_exif, source};
mod gradient_raster;
mod svg_fragment;

pub(super) mod exact_layer;
mod radius_lengths;
mod tile_positions;
#[cfg(test)]
use crate::paint::background::tile_positions::tiling;
mod shape_path;
pub use crate::paint::background::shape_path::*;
mod shape_raster;
pub use crate::paint::background::shape_raster::*;
pub(super) mod shape_profile;
pub use crate::paint::background::shape_profile::*;
pub(super) mod raster;
pub(crate) use crate::paint::background::raster::*;
pub(super) mod image_decode;
pub use crate::paint::background::image_decode::*;
pub(super) mod tiles;
pub use crate::paint::background::tiles::*;

type Cache = Mutex<HashMap<String, Option<Source>>>;
static CACHE: OnceLock<Cache> = OnceLock::new();

/// Сколько разных картинок держим декодированными.
const CACHE_CAP: usize = 32;

/// Потолок на число плиток вдоль оси: битый `background-size` иначе просит
/// миллионы копий.
const MAX_TILES: f32 = 2048.0;

/// Декодировать по ссылке из `url(...)`: `data:`-URI или путь на диске.
///
/// Сеть не трогаем по тем же причинам, что и в элементе-картинке: документ
/// рисуется в чате, где загрузка чужих адресов недопустима.
pub fn load(src: &str) -> Option<Arc<RenderImage>> {
    match source(src)? {
        Source::Raster(image) => Some(image),
        // Своя величина рисунка — то же, что для растра: он растрируется под
        // неё, а нужный размер плитки посчитает вызывающий.
        Source::Vector { markup, size } => {
            let (w, h) = default_size(size, (300.0, 150.0));
            crate::svg::raster::rasterize(&markup, w, h)
        }
        Source::Gradient { raw } => rasterize_gradient(&raw, 300, 150),
        Source::Shape { raw } => rasterize_shape(&raw, 300, 150, 1.0),
    }
}

/// Своя величина картинки в CSS-точках (css-images-3 §5.1, недостающее —
/// §5.3 от места 300×150, как у `load`).
///
/// Размер растра `load` для этого не годится: рисунок растрируется с
/// плотностью `svg::DENSITY` (2×), и флоат-картинка с формой из 100-точечного
/// SVG заводилась 200×200 (`shape-image-002`: вырез на всю ширину
/// контейнера, `-017`: контейнер вдвое выше). У растра величина и есть его
/// точки — там ничего не меняется.
pub fn intrinsic_px(src: &str) -> Option<(f32, f32)> {
    Some(default_size(source(src)?.intrinsic(), (300.0, 150.0)))
}

/// Чем задана фоновая картинка: готовым растром или разметкой рисунка.
///
/// Рисунок нельзя раскодировать раз и навсегда: у него нет своих точек, и
/// растрировать его надо ПОД РАЗМЕР ПЛИТКИ — иначе он выходит мыльным при
/// увеличении и лишним расходом при уменьшении.
#[derive(Clone)]
pub enum Source {
    Raster(Arc<RenderImage>),
    Vector {
        markup: String,
        size: Intrinsic,
    },
    /// Градиент: своей величины НЕТ вовсе (css-images-3 §4.4) — обе оси
    /// берутся от области, а растрируется он точно в размер плитки.
    Gradient {
        raw: String,
    },
    /// Базовая форма `clip-path` (`circle`/`ellipse`): альфа-маска буфера
    /// группы. Радиусы и центр считаются от размера плитки (= коробки).
    Shape {
        raw: String,
    },
}

impl Source {
    pub(crate) fn mask_raster(&self, tile: (f32, f32), scale: f32) -> Option<Arc<RenderImage>> {
        if let Source::Gradient { raw } = self
            && !raw.starts_with("cross-fade(")
            // Конический градиент и цвет-изображение растрируются своими путями
            // (`conic`, `sources::raster_color`) — им обычный путь плитки.
            && !conic::is_conic(raw)
            && crate::style::computed::parse_image_color(raw).is_none()
        {
            // CSS Masking section 7.8 uses CSS image sizing, but coverage is sampled
            // at device pixel centres. Avoid resizing a CSS-resolution alpha
            // bitmap across sharp stops at fractional window scales.
            let css = (tile.0.clamp(1.0, 2048.0), tile.1.clamp(1.0, 2048.0));
            let w = (css.0 * scale).round().clamp(1.0, 4096.0) as u32;
            let h = (css.1 * scale).round().clamp(1.0, 4096.0) as u32;
            return gradient_raster::raster(raw, w, h, css, true);
        }
        self.raster(tile)
    }

    /// Своя величина картинки.
    pub fn intrinsic(&self) -> Intrinsic {
        match self {
            Source::Raster(image) => {
                let s = image.size(0);
                let (w, h) = ((s.width.0 as f32).max(1.0), (s.height.0 as f32).max(1.0));
                Intrinsic {
                    w: Some(w),
                    h: Some(h),
                    ratio: Some(w / h),
                }
            }
            Source::Vector { size, .. } => *size,
            Source::Gradient { .. } | Source::Shape { .. } => Intrinsic {
                w: None,
                h: None,
                ratio: None,
            },
        }
    }

    /// Растр под нужный размер плитки.
    pub fn raster(&self, tile: (f32, f32)) -> Option<Arc<RenderImage>> {
        match self {
            Source::Raster(image) => Some(image.clone()),
            Source::Vector { markup, .. } => {
                // Растр не бывает больше потолка: при `cover` с вытянутым
                // соотношением плитка выходит в тысячи точек по длинной
                // стороне, и растеризатор возвращал НИЧЕГО — страница
                // оставалась пустой (`wide--cover--*`, `tall--cover--*`).
                // Геометрия при этом не страдает: плитка рисуется своим
                // размером, теряется только плотность, а видна всё равно
                // только та её часть, что попала в коробку.
                const LIMIT: f32 = 2048.0;
                // Потолок по КАЖДОЙ оси отдельно: общий коэффициент при
                // крайнем соотношении (`cover` на плитке 96000x330) сжимал
                // короткую сторону до считанных точек, и растянутый обратно
                // растр мылил заливку в белёсость. Пропорции растра при этом
                // ломаются — но видима лишь часть плитки в коробке, а сам
                // рисунок растрируется в свою область просмотра целиком.
                let raster = (tile.0.clamp(1.0, LIMIT), tile.1.clamp(1.0, LIMIT));
                crate::svg::raster::rasterize(&with_viewport(markup, raster), raster.0, raster.1)
            }
            Source::Gradient { raw } => {
                const LIMIT: f32 = 2048.0;
                let (w, h) = (
                    tile.0.clamp(1.0, LIMIT).round() as u32,
                    tile.1.clamp(1.0, LIMIT).round() as u32,
                );
                rasterize_gradient(raw, w, h)
            }
            Source::Shape { raw } => {
                const LIMIT: f32 = 2048.0;
                let (w, h) = (
                    tile.0.clamp(1.0, LIMIT).round() as u32,
                    tile.1.clamp(1.0, LIMIT).round() as u32,
                );
                rasterize_shape(raw, w, h, 1.0)
            }
        }
    }
}

/// Разбить запись по пробелам ВЕРХНЕГО уровня: `calc(10px + 15%)` — один
/// токен. Без скобок — ровно `split_whitespace`.
pub(crate) fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        if ch.is_whitespace() && depth <= 0 {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

/// Слой фоновой картинки: канвас, рисующий плитки внутри своих границ.
/// Коробка ПОЗИЦИОНИРОВАНИЯ корня — отступы её краёв от краёв холста.
///
/// Хранится отступами, а не готовым прямоугольником: при `width: auto` (а так
/// во всей семье `background-root-*`) размер известен только на отрисовке,
/// когда виден холст.
#[derive(Clone, Copy, Default)]
pub struct RootArea {
    /// Поле плюс рамка корня с этой стороны.
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    /// Padding-box корня, если размер ЗАДАН: размер плюс отступы по оси.
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// `vertical-rl`: заданная ширина отмеряется от ПРАВОГО края холста.
    pub from_right: bool,
    /// Ключ замера левого края padding-box корня (`interact::root_left_prev`):
    /// корень `vertical-rl` без заданной ширины — по содержимому и прижат
    /// вправо, его край известен только после раскладки. Читается при
    /// ОТРИСОВКЕ: подготовка тела (`interact::RecordRootLeft`) идёт раньше
    /// отрисовки холста в том же кадре.
    pub left_key: Option<u64>,
}

impl RootArea {
    pub(crate) fn rect(&self, clip: Bounds<Pixels>) -> Bounds<Pixels> {
        let (cw, ch) = (f32::from(clip.size.width), f32::from(clip.size.height));
        let w = self.width.unwrap_or(cw - self.left - self.right).max(0.0);
        let h = self.height.unwrap_or(ch - self.top - self.bottom).max(0.0);
        let left_abs = self
            .left_key
            .filter(|_| self.width.is_none())
            .and_then(crate::text::vertical::root_left_prev)
            .map(|l| l - f32::from(clip.origin.x));
        let (x, w) = match left_abs {
            Some(l) => (l, (cw - l - self.right).max(0.0)),
            None => (self.left_x(cw, w), w),
        };
        Bounds {
            origin: gpui::point(clip.origin.x + px(x), clip.origin.y + px(self.top)),
            size: gpui::size(px(w), px(h)),
        }
    }

    pub(crate) fn left_x(&self, cw: f32, w: f32) -> f32 {
        if self.from_right && self.width.is_some() {
            cw - self.right - w
        } else {
            self.left
        }
    }
}

/// Слой фона КАНВАСА: плитки меряются коробкой корня, а красят весь холст.
pub fn canvas_layer(c: &Computed, area: RootArea) -> Option<AnyElement> {
    c.bg_image.as_ref()?;
    let style = c.clone();
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |clip: Bounds<Pixels>, _, window, _| {
                paint_tiles(&style, area.rect(clip), Some(clip), window);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// Заливка `background-clip: border-area` (css-backgrounds-4): фон «within
/// the area painted by the border» — одноцветный фон тогда просто лежит ПОД
/// краской рамки. Рисуют его те же примитивы, что и рамку (квад, слой рамки,
/// кольца `corner-shape` и `border-shape`), с той же геометрией стиля.
fn border_area_fill(c: &Computed) -> Option<crate::style::values::value::Color> {
    if c.bg_clip != Some(crate::style::computed::BgClip::BorderArea) {
        return None;
    }
    flat_fill(c)
}

/// Цвет, которым красится рамка: свой `border-color` поверх заливки
/// `border-area` («ignoring any transparency introduced by border-color»).
pub(crate) fn border_paint(
    c: &Computed,
    colour: crate::style::values::value::Color,
) -> crate::style::values::value::Color {
    match border_area_fill(c) {
        Some(fill) => over(colour, fill),
        None => colour,
    }
}

pub fn layer(c: &Computed) -> Option<AnyElement> {
    c.bg_image.as_ref()?;
    // Одноцветный фон `border-area` несёт краска рамки (`border_paint`):
    // плитки легли бы на всю коробку.
    if border_area_fill(c).is_some() {
        return None;
    }
    let style = c.clone();
    Some(
        exact_layer::ExactLayer::new(move |bounds: Bounds<Pixels>, window: &mut gpui::Window| {
            if style.bg_fixed == Some(true) {
                // Плитка меряется и отсчитывается от ОБЛАСТИ ПРОСМОТРА,
                // а красится только внутри своей коробки: сдвиг между
                // ними держит сам `paint_tiles`.
                let view = Bounds {
                    origin: gpui::point(px(0.0), px(0.0)),
                    size: window.viewport_size(),
                };
                paint_tiles(&style, view, Some(bounds), window);
            } else {
                paint_area(&style, bounds, window);
            }
        })
        .into_any_element(),
    )
}

// --- Сплошная заливка ----------------------------------------------------
//
// Фон, который сводится к ОДНОМУ цвету (цвет фона, одноцветный градиент,
// растр из одинаковых непрозрачных точек, мощённый без зазоров), не
// нуждается в маске для `background-clip: text | border-area`: области
// краски хватает цвета глифа или рамки.

/// Краска `top` поверх `base` (source-over), цвета без премультипликации.
pub(crate) fn over(
    top: crate::style::values::value::Color,
    base: crate::style::values::value::Color,
) -> crate::style::values::value::Color {
    let a = top.a + base.a * (1.0 - top.a);
    if a <= 0.0 {
        return crate::style::values::value::Color::default();
    }
    let mix = |t: f32, b: f32| (t * top.a + b * base.a * (1.0 - top.a)) / a;
    crate::style::values::value::Color {
        r: mix(top.r, base.r),
        g: mix(top.g, base.g),
        b: mix(top.b, base.b),
        a,
    }
}

/// Единственный цвет растра, если все его точки одинаковы и непрозрачны.
fn flat_colour(src: &str) -> Option<crate::style::values::value::Color> {
    let Source::Raster(image) = source(src)? else {
        return None;
    };
    let bytes = image.as_bytes(0)?;
    let first = bytes.get(0..4)?;
    if first[3] != 255 || !bytes.chunks_exact(4).all(|p| p == first) {
        return None;
    }
    // Порядок байтов растра — BGRA (см. `border_image::tests`).
    Some(crate::style::values::value::Color {
        r: first[2] as f32 / 255.0,
        g: first[1] as f32 / 255.0,
        b: first[0] as f32 / 255.0,
        a: 1.0,
    })
}

/// Весь фон коробки одним цветом, если он таков: цвет фона, поверх него
/// одноцветный градиент или одноцветный растр, мощённый без зазоров.
/// `None` — фон узорный (или его нет вовсе).
fn flat_fill(c: &Computed) -> Option<crate::style::values::value::Color> {
    if c.gradient.is_some() && c.bg_image.is_some() {
        return None;
    }
    let mut fill = c.background.unwrap_or_default();
    if let Some(g) = &c.gradient {
        let one = g.from;
        let flat = !c.gradient_as_tile()
            && g.to == one
            && g.stops.iter().all(|s| s.0 == one)
            && g.stops_px.iter().all(|s| s.0 == one)
            && g.stops_raw.iter().all(|s| s.0 == one);
        if !flat {
            return None;
        }
        fill = over(one, fill);
    }
    if let Some(src) = &c.bg_image {
        let rep = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
        let covers = |t: Tiling| matches!(t, Tiling::Repeat | Tiling::Round);
        if !covers(rep.axis(true)) || !covers(rep.axis(false)) {
            return None;
        }
        fill = over(flat_colour(&key_exif(src, c))?, fill);
    }
    (fill.a > 0.0).then_some(fill)
}

/// Заливка `background-clip: text`, которую несёт цвет глифов.
///
/// Только НЕПРОЗРАЧНАЯ: тогда «цвет поверх заливки» тоже непрозрачен, и
/// повторное слияние стилей (`inline::inherit` зовётся цепочкой) даёт тот же
/// цвет — смешение не копится.
pub(crate) fn text_clip_fill(c: &Computed) -> Option<crate::style::values::value::Color> {
    if c.bg_clip != Some(crate::style::computed::BgClip::Text) {
        return None;
    }
    flat_fill(c).filter(|f| f.a >= 1.0)
}

/// Нарисовать фоновые плитки стиля в ЗАДАННОЙ области.
///
/// Отдельной функцией, а не замыканием слоя: фон РЯДА таблицы рисуется от
/// области ряда, но обрезается прямоугольниками ячеек — вызывающий ставит
/// маску сам и зовёт отрисовку с областью ряда.
pub fn paint_area(c: &Computed, bounds: Bounds<Pixels>, window: &mut gpui::Window) {
    // `background-attachment: fixed` и здесь считается от ОБЛАСТИ ПРОСМОТРА,
    // а красится внутри своей области — та же двухобластная модель, что у
    // обычной коробки (`layer`). Без неё полоса ряда и группы клала плитку от
    // своего верха и уезжала вниз на всю свою высоту
    // (`background-attachment-applies-to-004/005/006`).
    if c.bg_fixed == Some(true) {
        let view = Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: window.viewport_size(),
        };
        paint_tiles(c, view, Some(bounds), window);
        return;
    }
    paint_tiles(c, bounds, None, window);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiling_starts_before_the_box_and_covers_it() {
        // Смещение 30 при плитке 20: первая копия обязана начаться левее нуля,
        // иначе между краем коробки и первой плиткой остаётся дыра.
        let xs = tiling(Tiling::Repeat, 30.0, 20.0, 100.0);
        let first = xs[0];
        assert!(first <= 0.0, "первая плитка начинается не правее коробки");
        assert!(
            first + xs.len() as f64 * 20.0 >= 100.0,
            "плитки обязаны закрыть коробку целиком"
        );
    }

    #[test]
    fn without_repeat_there_is_exactly_one_copy() {
        assert_eq!(tiling(Tiling::None, 12.0, 20.0, 100.0), vec![12.0]);
    }

    /// `space` раздаёт остаток РАВНЫМИ зазорами, а крайние плитки прижимает к
    /// краям (css-backgrounds-3 §3.4).
    #[test]
    fn space_pins_the_edges_and_shares_the_rest() {
        let xs = tiling(Tiling::Space, 0.0, 32.0, 106.0);
        assert_eq!(xs.len(), 3, "целых плиток влезает три");
        assert_eq!(xs[0], 0.0);
        assert!((xs[2] + 32.0 - 106.0).abs() < 0.01, "последняя у края");
    }

    /// `round` подгоняет САМУ плитку под целое их число.
    #[test]
    fn round_fits_a_whole_number_of_tiles() {
        assert_eq!(rounded(Tiling::Round, 30.0, 100.0), 100.0 / 3.0);
        assert_eq!(rounded(Tiling::Repeat, 30.0, 100.0), 30.0);
    }

    /// Умолчальный размер: доля своей стороной не является, и рисунок
    /// занимает место под фон целиком (css-images-3 §5.3).
    #[test]
    fn default_size_falls_back_to_the_area() {
        let none = Intrinsic::default();
        assert_eq!(default_size(none, (256.0, 768.0)), (256.0, 768.0));
        let ratio = Intrinsic {
            ratio: Some(2.0),
            ..Default::default()
        };
        assert_eq!(default_size(ratio, (200.0, 400.0)), (200.0, 100.0));
        let sides = Intrinsic {
            w: Some(60.0),
            h: Some(30.0),
            ratio: Some(2.0),
        };
        assert_eq!(default_size(sides, (200.0, 400.0)), (60.0, 30.0));
    }

    /// Своя величина рисунка: доля стороной не считается, `viewBox` даёт
    /// только соотношение.
    #[test]
    fn svg_percent_side_is_not_intrinsic() {
        let i = svg_size("<svg xmlns=\"…\" height=\"50%\"></svg>");
        assert_eq!(i, Intrinsic::default());
        let i = svg_size("<svg viewBox=\"0 0 2560 208\"></svg>");
        assert_eq!(i.w, None);
        assert_eq!(i.ratio, Some(2560.0 / 208.0));
    }

    #[test]
    fn base64_reads_a_known_payload() {
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
    }
}
