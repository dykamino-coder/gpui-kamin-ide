//! Декодирование картинок: EXIF-ориентация, SVG-размер, data:-URL, собственный размер.

mod svg_metadata;
use svg_metadata::degenerate_viewbox;
pub(crate) use svg_metadata::svg_root_background;
pub(super) use svg_metadata::svg_size;

mod orientation;
use orientation::exif_orientation;
use orientation::orient_image;

use crate::paint::background::*;

/// Цвет градиента в точке `t` (0..1) по расставленным стопам.
/// Растр или рисунок — по содержимому файла, а не по расширению: у `data:`-URI
/// расширения нет вовсе.
pub(super) fn decode(bytes: &[u8], orient: bool) -> Option<Source> {
    // Ищем корневой тег, а не начало файла: перед ним стоят и объявление XML,
    // и комментарий с лицензией — с них начинается добрая половина рисунков
    // набора (`background-size/vector/support/*`). Окно широкое: комментарий
    // в `colors-16x8-parDefault.svg` длиннее 512 байт, и рисунок не
    // распознавался вовсе.
    let head = &bytes[..bytes.len().min(4096)];
    let looks_svg = std::str::from_utf8(head)
        .ok()
        .map(|t| t.contains("<svg"))
        .unwrap_or(false);
    if looks_svg {
        let markup = String::from_utf8(bytes.to_vec()).ok()?;
        // Вырожденная область просмотра (нулевая ось `viewBox`) — картинки
        // НЕТ вовсе (SVG intrinsic sizing): браузер такой фон не рисует.
        if degenerate_viewbox(&markup) {
            return None;
        }
        let size = svg_size(&markup);
        return Some(Source::Vector { markup, size });
    }
    let image = gpui::raster_bytes_to_image(bytes)?;
    // Вшитый цветовой профиль (PNG `iCCP`) — часть картинки: её точки заданы
    // в ЕГО пространстве (css-color-4 §12, tagged images).
    let image = match gpui::png_icc_profile(bytes)
        .and_then(|profile| crate::style::values::color_space::apply_icc(&image, &profile))
    {
        Some(fixed) => fixed,
        None => image,
    };
    // Разворот по EXIF (css-images-3 §5.4, начальное значение `from-image`):
    // «All CSS layout and rendering processes use the image AFTER rotation…
    // The natural height and width are derived from the rotated rather than
    // the original image dimensions». Значит применять надо ЗДЕСЬ, до того как
    // кто-нибудь спросит `Source::intrinsic()`, — как Blink разворачивает в
    // `LayoutImageResource::ImageOrientation`.
    if orient
        && let Some(tag) = exif_orientation(bytes)
        && tag > 1
        && let Some(turned) = orient_image(&image, tag)
    {
        return Some(Source::Raster(turned));
    }
    Some(Source::Raster(image))
}

// Подставить корню SVG `viewBox` его `<view id="…">` (SVG 2 §8.2). Не SVG
// или вида нет — байты как есть.

pub(super) fn read_bytes(src: &str) -> Option<Vec<u8>> {
    if let Some(rest) = src.strip_prefix("data:") {
        let (head, payload) = rest.split_once(',')?;
        // RFC 2397: без пометки `base64` содержимое лежит прямо в адресе,
        // процентно-кодированным (`%3Csvg…`). Такое встречается у рисунков.
        if head.ends_with("base64") {
            return base64_decode(payload);
        }
        return Some(percent_decode(payload));
    }
    let path = src.strip_prefix("file:///").unwrap_or(src);
    std::fs::read(path).ok()
}

/// Процентное кодирование адресов: `%3C` → `<`. Остальные знаки как есть.
pub(super) fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// Base64 без зависимости: нужен ровно один раз и только на чтение.
pub(super) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for ch in text.bytes() {
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Своя величина картинки: стороны и соотношение — каждое, только если есть.
///
/// У растра есть всё. У рисунка бывает что угодно: `width="50%"` своей
/// величиной НЕ является (доля считается от места под фон, а не от картинки),
/// а `viewBox` даёт одно соотношение без сторон. От этого набора и зависит,
/// каким выйдет размер плитки при `background-size: auto`.
///
/// Величина в CSS — это размер в ТОЧКАХ САМОЙ КАРТИНКИ (css-images-3 §4.1):
/// изображение 60×60 занимает 60×60 точек CSS при любом масштабе дисплея.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intrinsic {
    pub w: Option<f32>,
    pub h: Option<f32>,
    /// Ширина, делённая на высоту.
    pub ratio: Option<f32>,
}

/// Умолчальный размер картинки (css-images-3 §5.3).
///
/// Недостающие стороны берутся из соотношения, а когда и его нет — из места
/// под фон. На этом стоит вся папка `background-size/vector`: рисунок с
/// долевым размером своих сторон не имеет вовсе и обязан занять место под
/// фон целиком.
pub(super) fn default_size(i: Intrinsic, area: (f32, f32)) -> (f32, f32) {
    match (i.w, i.h, i.ratio) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some(r)) if r > 0.0 => (w, w / r),
        (None, Some(h), Some(r)) => (h * r, h),
        (Some(w), None, None) => (w, area.1),
        (None, Some(h), None) => (area.0, h),
        // Только соотношение — вписываемся в место под фон, сохраняя его.
        (None, None, Some(r)) if r > 0.0 => {
            let k = (area.0 / r).min(area.1);
            (k * r, k)
        }
        _ => area,
    }
}
