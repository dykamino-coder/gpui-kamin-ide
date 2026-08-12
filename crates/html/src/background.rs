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

use crate::computed::{BgPos, BgRepeat, BgSize, Computed, Tiling};
use crate::value::Len;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, RenderImage, Styled, px};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

type Cache = Mutex<HashMap<String, Option<Arc<RenderImage>>>>;
static CACHE: OnceLock<Cache> = OnceLock::new();

/// Сколько разных картинок держим декодированными.
const CACHE_CAP: usize = 32;

/// Декодировать по ссылке из `url(...)`: `data:`-URI или путь на диске.
///
/// Сеть не трогаем по тем же причинам, что и в элементе-картинке: документ
/// рисуется в чате, где загрузка чужих адресов недопустима.
pub fn load(src: &str) -> Option<Arc<RenderImage>> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(src)
    {
        return hit.clone();
    }
    let bytes = read_bytes(src);
    let image = bytes.as_deref().and_then(gpui::raster_bytes_to_image);
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(src.to_string(), image.clone());
    }
    image
}

fn read_bytes(src: &str) -> Option<Vec<u8>> {
    if let Some(rest) = src.strip_prefix("data:") {
        let payload = rest.split_once("base64,")?.1;
        return base64_decode(payload);
    }
    let path = src.strip_prefix("file:///").unwrap_or(src);
    std::fs::read(path).ok()
}

/// Base64 без зависимости: нужен ровно один раз и только на чтение.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
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

/// Размер одной плитки в точках по правилам `background-size`.
fn tile_size(
    image: &RenderImage,
    box_size: (f32, f32),
    size: BgSize,
    scale_factor: f32,
) -> (f32, f32) {
    let s = image.size(0);
    // Размер образа приходит в ФИЗИЧЕСКИХ точках, а коробка — в логических:
    // без деления на масштаб дисплея плитка на 150% выходила в полтора раза
    // крупнее браузерной.
    let k = scale_factor.max(0.01);
    let (iw, ih) = (
        (s.width.0 as f32 / k).max(1.0),
        (s.height.0 as f32 / k).max(1.0),
    );
    let (bw, bh) = box_size;
    match size {
        BgSize::Auto => (iw, ih),
        BgSize::Cover | BgSize::Contain => {
            let sx = bw / iw;
            let sy = bh / ih;
            // `cover` закрывает коробку целиком, `contain` вписывается в неё.
            let k = if matches!(size, BgSize::Cover) {
                sx.max(sy)
            } else {
                sx.min(sy)
            };
            (iw * k, ih * k)
        }
        // Заданная одна сторона тянет вторую пропорционально — как в CSS.
        BgSize::Fixed(w, h) => match (len_px(w, bw), len_px(h, bh)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, ih * w / iw),
            (None, Some(h)) => (iw * h / ih, h),
            (None, None) => (iw, ih),
        },
    }
}

fn len_px(l: Option<Len>, base: f32) -> Option<f32> {
    match l? {
        Len::Px(v) => Some(v),
        Len::Pct(v) => Some(base * v),
        Len::Em(k) => Some(k * 16.0),
        Len::Ch(k) => Some(k * crate::metrics::ch_ex_px("", 16.0).0),
        Len::Ic(k) => Some(k * crate::metrics::ic_px("", 16.0)),
        Len::Ex(k) => Some(k * crate::metrics::ch_ex_px("", 16.0).1),
        Len::Vw(_) | Len::Vh(_) => None,
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
    }
}

/// Смещение первой плитки: проценты считаются от свободного места, как в CSS.
fn origin(pos: BgPos, box_size: (f32, f32), tile: (f32, f32)) -> (f32, f32) {
    let axis = |l: Option<Len>, box_len: f32, tile_len: f32| -> f32 {
        match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(v)) => (box_len - tile_len) * v,
            _ => 0.0,
        }
    };
    (
        axis(pos.x, box_size.0, tile.0),
        axis(pos.y, box_size.1, tile.1),
    )
}

/// Слой фоновой картинки: канвас, рисующий плитки внутри своих границ.
pub fn layer(c: &Computed) -> Option<AnyElement> {
    let src = c.bg_image.clone()?;
    let size = c.bg_size;
    let pos = c.bg_pos;
    let repeat = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
    let radius = match c.radius.tl {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let Some(image) = load(&src) else { return };
                let box_size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let tile = tile_size(&image, box_size, size, window.scale_factor());
                if tile.0 <= 0.5 || tile.1 <= 0.5 {
                    return;
                }
                // `round` меняет САМ размер плитки, поэтому считается до
                // смещения: доля в `background-position` берётся уже от
                // подогнанной плитки.
                let tile = (
                    rounded(repeat.axis(true), tile.0, box_size.0),
                    rounded(repeat.axis(false), tile.1, box_size.1),
                );
                let start = origin(pos, box_size, tile);
                // Каждая ось укладывается по своему правилу: сплошняком,
                // одной плиткой, целым числом с равными зазорами (`space`)
                // или подогнанной плиткой (`round`).
                let xs = tiling(repeat.axis(true), start.0, tile.0, box_size.0);
                let ys = tiling(repeat.axis(false), start.1, tile.1, box_size.1);
                let corners = gpui::Corners::all(px(radius));
                window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                    for y in &ys {
                        for x in &xs {
                            let at = gpui::point(
                                bounds.origin.x + px(*x),
                                bounds.origin.y + px(*y),
                            );
                            let cell = Bounds {
                                origin: at,
                                size: gpui::size(px(tile.0), px(tile.1)),
                            };
                            // Промах атласа рисовать нечем — пропускаем плитку молча.
                            let _ = window.paint_image(cell, corners, image.clone(), 0, false);
                        }
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// Размер плитки после подгонки под целое их число (`background-repeat: round`).
///
/// css-backgrounds-3 §3.4: плитка растягивается или сжимается так, чтобы вдоль
/// оси уложилось целое их число без зазоров. Одна плитка — минимум: меньше
/// целой копии не бывает.
fn rounded(mode: Tiling, tile: f32, box_len: f32) -> f32 {
    if mode != Tiling::Round || tile <= 0.0 || box_len <= 0.0 {
        return tile;
    }
    let count = (box_len / tile).round().max(1.0);
    box_len / count
}

/// Координаты плиток вдоль оси — по одной на каждую копию.
///
/// Возврат списком, а не парой «сколько и откуда»: при `space` плитки стоят
/// НЕ через равные шаги в размер плитки, а через зазор, и одной формулой
/// смещения их уже не описать.
fn tiling(mode: Tiling, start: f32, tile: f32, box_len: f32) -> Vec<f32> {
    // Потолок на число плиток: битый `background-size` иначе просит миллионы.
    const MAX: f32 = 512.0;
    match mode {
        Tiling::None => vec![start],
        // Зазоры раздаются между ЦЕЛЫМИ плитками, крайние прижаты к краям, а
        // `background-position` вдоль этой оси не действует. Если целиком
        // влезает меньше двух — плитка одна и смещение своё (§3.4).
        Tiling::Space => {
            let fit = (box_len / tile).floor();
            if fit < 2.0 {
                return vec![start];
            }
            let count = fit.min(MAX);
            let gap = (box_len - count * tile) / (count - 1.0);
            (0..count as u32)
                .map(|i| i as f32 * (tile + gap))
                .collect()
        }
        // `round` уже подогнал размер плитки — дальше это обычная кладка.
        Tiling::Repeat | Tiling::Round => {
            // Начало сдвигается назад на целое число плиток, иначе смещение
            // съедало бы первый ряд.
            let back = (start / tile).ceil();
            let first = start - back * tile;
            let count = ((box_len - first) / tile).ceil().max(1.0).min(MAX);
            (0..count as u32).map(|i| first + i as f32 * tile).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiling_starts_before_the_box_and_covers_it() {
        // Смещение 30 при плитке 20: первая копия обязана начаться левее нуля,
        // иначе между краем коробки и первой плиткой остаётся дыра.
        let (count, first) = tiling(true, 30.0, 20.0, 100.0);
        assert!(first <= 0.0, "первая плитка начинается не правее коробки");
        assert!(
            first + count as f32 * 20.0 >= 100.0,
            "плитки обязаны закрыть коробку целиком"
        );
    }

    #[test]
    fn without_repeat_there_is_exactly_one_copy() {
        assert_eq!(tiling(false, 12.0, 20.0, 100.0), (1, 12.0));
    }

    #[test]
    fn base64_reads_a_known_payload() {
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
    }
}
