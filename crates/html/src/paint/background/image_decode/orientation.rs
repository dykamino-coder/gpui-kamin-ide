//! Ориентация декодированного растра по EXIF без изменения декодирования.

use gpui::RenderImage;
use std::sync::Arc;

/// Метка `Orientation` (TIFF-тег 0x0112) из EXIF: JPEG `APP1` или PNG `eXIf`.
///
/// Значения 1..8 по TIFF 6.0; всё прочее (в том числе «9» из набора) — как
/// `none`, потому что §5.4 велит невнятную метку считать отсутствующей.
pub(super) fn exif_orientation(bytes: &[u8]) -> Option<u16> {
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
pub(super) fn orient_image(image: &Arc<RenderImage>, tag: u16) -> Option<Arc<RenderImage>> {
    let s = image.size(0);
    let (w, h) = (s.width.0 as u32, s.height.0 as u32);
    let bytes = image.as_bytes(0)?;
    // 5..8 меняют оси местами — у развёрнутой картинки другой природный размер.
    let swap = matches!(tag, 5..=8);
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
