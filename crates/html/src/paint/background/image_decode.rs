//! Декодирование картинок: EXIF-ориентация, SVG-размер, data:-URL, собственный размер.

use crate::paint::background::*;

/// Цвет градиента в точке `t` (0..1) по расставленным стопам.
/// Растр или рисунок — по содержимому файла, а не по расширению: у `data:`-URI
/// расширения нет вовсе.
pub(crate) fn decode(bytes: &[u8], orient: bool) -> Option<Source> {
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
        .and_then(|profile| crate::color_space::apply_icc(&image, &profile))
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

/// Метка `Orientation` (TIFF-тег 0x0112) из EXIF: JPEG `APP1` или PNG `eXIf`.
///
/// Значения 1..8 по TIFF 6.0; всё прочее (в том числе «9» из набора) — как
/// `none`, потому что §5.4 велит невнятную метку считать отсутствующей.
pub(crate) fn exif_orientation(bytes: &[u8]) -> Option<u16> {
    // Найти блок TIFF: у JPEG он лежит за «Exif\0\0» в сегменте APP1, у PNG —
    // телом куска `eXIf`.
    let tiff = if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        loop {
            if i + 4 > bytes.len() || bytes[i] != 0xFF {
                return None;
            }
            let marker = bytes[i + 1];
            // Начало сжатых данных — дальше сегментов нет.
            if marker == 0xDA || marker == 0xD9 {
                return None;
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            let body = bytes.get(i + 4..i + 2 + len)?;
            if marker == 0xE1 && body.starts_with(b"Exif\0\0") {
                break &body[6..];
            }
            i += 2 + len;
        }
    } else {
        let mut i = 8usize;
        loop {
            let len = u32::from_be_bytes(*bytes.get(i..i + 4)?.first_chunk()?) as usize;
            let kind = bytes.get(i + 4..i + 8)?;
            if kind == b"eXIf" {
                break bytes.get(i + 8..i + 8 + len)?;
            }
            // `eXIf` ПОСЛЕ данных изображения не действует: PNG 3rd ed.
            // §11.3.6 «The eXIf chunk … shall be before the first IDAT
            // chunk», и браузеры позднюю метку игнорируют
            // (`image-orientation-exif-png-2/3`: `F-exif-late.png` обязан
            // остаться неповёрнутым).
            if kind == b"IDAT" || kind == b"IEND" {
                return None;
            }
            i += 12 + len;
        }
    };
    let le = match tiff.first_chunk::<2>()? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let b = *tiff.get(at..at + 2)?.first_chunk()?;
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b = *tiff.get(at..at + 4)?.first_chunk()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = u16_at(ifd)? as usize;
    for n in 0..count {
        let at = ifd + 2 + n * 12;
        if u16_at(at)? == 0x0112 {
            return u16_at(at + 8);
        }
    }
    None
}

/// Развернуть растр по метке EXIF (TIFF 6.0, значения 1..8).
///
/// Точки переставляются целыми четвёрками — порядок каналов (BGRA,
/// премультиплицированный) при этом не важен, как и в `crop_image`.
pub(crate) fn orient_image(
    image: &Arc<RenderImage>,
    tag: u16,
) -> Option<Arc<RenderImage>> {
    let s = image.size(0);
    let (w, h) = (s.width.0 as u32, s.height.0 as u32);
    let bytes = image.as_bytes(0)?;
    // 5..8 меняют оси местами — у развёрнутой картинки другой природный размер.
    let swap = matches!(tag, 5 | 6 | 7 | 8);
    let (ow, oh) = if swap { (h, w) } else { (w, h) };
    let mut out = Vec::with_capacity((ow * oh * 4) as usize);
    for oy in 0..oh {
        for ox in 0..ow {
            let (sx, sy) = match tag {
                2 => (w - 1 - ox, oy),
                3 => (w - 1 - ox, h - 1 - oy),
                4 => (ox, h - 1 - oy),
                5 => (oy, ox),
                6 => (oy, h - 1 - ox),
                7 => (w - 1 - oy, h - 1 - ox),
                8 => (w - 1 - oy, ox),
                _ => (ox, oy),
            };
            let at = ((sy * w + sx) * 4) as usize;
            out.extend_from_slice(bytes.get(at..at + 4)?);
        }
    }
    gpui::bgra_bytes_to_image(ow, oh, out)
}

/// Своя величина рисунка: `width`/`height` корневого тега, иначе `viewBox`.
///
/// Разбирается по тексту, а не деревом: дерево документа рисунка нам не нужно
/// нигде больше, а растеризатору всё равно идёт исходная разметка.
/// Нулевая ось `viewBox`: соотношение вырождено, рисовать нечего.
pub(crate) fn degenerate_viewbox(markup: &str) -> bool {
    let head = match markup.find("<svg") {
        Some(at) => {
            &markup[at..markup[at..]
                .find('>')
                .map(|e| at + e)
                .unwrap_or(markup.len())]
        }
        None => return false,
    };
    let Some(at) = head.find("viewBox=") else {
        return false;
    };
    let rest = head[at + 8..].trim_start();
    let Some(quote) = rest.chars().next() else {
        return false;
    };
    let Some(vb) = rest[1..].split(quote).next() else {
        return false;
    };
    let nums: Vec<f32> = vb
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    nums.len() == 4 && (nums[2] <= 0.0 || nums[3] <= 0.0)
}

/// Фон КАНВЫ рисунка: `style="background: …"` на корневом `<svg>`.
///
/// Это CSS-свойство замещаемого корня, а не SVG-контент — растеризатор его
/// не рисует, и рисунок из одного фона выходил прозрачным (box-sizing-007).
pub(crate) fn svg_root_background(markup: &str) -> Option<crate::value::Color> {
    let head = match markup.find("<svg") {
        Some(at) => {
            &markup[at..markup[at..]
                .find('>')
                .map(|e| at + e)
                .unwrap_or(markup.len())]
        }
        None => return None,
    };
    let at = head.find("style=")?;
    let rest = head[at + 6..].trim_start();
    let quote = rest.chars().next()?;
    let style = rest[1..].split(quote).next()?;
    let decls = crate::css::parse_decls(style);
    let v = decls
        .get("background")
        .or_else(|| decls.get("background-color"))?;
    crate::value::Color::parse(v.split_whitespace().next()?)
}

pub(crate) fn svg_size(markup: &str) -> Intrinsic {
    let head = match markup.find("<svg") {
        Some(at) => {
            &markup[at..markup[at..]
                .find('>')
                .map(|e| at + e)
                .unwrap_or(markup.len())]
        }
        None => markup,
    };
    let raw = |name: &str| -> Option<&str> {
        let at = head.find(&format!("{name}="))?;
        let rest = head[at + name.len() + 1..].trim_start();
        let quote = rest.chars().next()?;
        Some(rest[1..].split(quote).next()?.trim())
    };
    // Доля СВОЕЙ величиной не является: она считается от места под фон, то
    // есть сторона у рисунка отсутствует (SVG §7.2 и css-images-3 §4.1).
    let side = |name: &str| -> Option<f32> {
        let v = raw(name)?;
        if v.ends_with('%') {
            return None;
        }
        v.trim_end_matches("px")
            .parse()
            .ok()
            .filter(|n: &f32| *n > 0.0)
    };
    let ratio = raw("viewBox").and_then(|vb| {
        let nums: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0).then(|| nums[2] / nums[3])
    });
    let (w, h) = (side("width"), side("height"));
    Intrinsic {
        w,
        h,
        // Обе стороны заданы — соотношение из них, иначе из `viewBox`.
        ratio: match (w, h) {
            (Some(w), Some(h)) => Some(w / h),
            _ => ratio,
        },
    }
}

/// Подставить корню SVG `viewBox` его `<view id="…">` (SVG 2 §8.2). Не SVG
/// или вида нет — байты как есть.

pub(crate) fn read_bytes(src: &str) -> Option<Vec<u8>> {
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
pub(crate) fn percent_decode(text: &str) -> Vec<u8> {
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
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
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
pub(crate) fn default_size(i: Intrinsic, area: (f32, f32)) -> (f32, f32) {
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
