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
            crate::svg::rasterize(&markup, w, h)
        }
    }
}

/// Чем задана фоновая картинка: готовым растром или разметкой рисунка.
///
/// Рисунок нельзя раскодировать раз и навсегда: у него нет своих точек, и
/// растрировать его надо ПОД РАЗМЕР ПЛИТКИ — иначе он выходит мыльным при
/// увеличении и лишним расходом при уменьшении.
#[derive(Clone)]
pub enum Source {
    Raster(Arc<RenderImage>),
    Vector { markup: String, size: Intrinsic },
}

impl Source {
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
                crate::svg::rasterize(&with_viewport(markup, raster), raster.0, raster.1)
            }
        }
    }
}

/// Разобрать ссылку в источник картинки; результат запоминается.
pub fn source(src: &str) -> Option<Source> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(src)
    {
        return hit.clone();
    }
    let found = if src.starts_with("linear-gradient(")
        || src.starts_with("radial-gradient(")
        || src.starts_with("conic-gradient(")
    {
        gradient_image(src)
    } else {
        read_bytes(src).as_deref().and_then(decode)
    };
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(src.to_string(), found.clone());
    }
    found
}

/// Задать рисунку область просмотра размером с плитку.
///
/// Область просмотра фонового рисунка — это его ПЛИТКА, а не что-то своё:
/// доли внутри рисунка (`height="50%"`, `<rect width="100%">`) считаются от
/// неё. Растеризатор же разбирает разметку как отдельный документ, и рисунок
/// с долевым размером корня не имеет для него размера вовсе — выходил пустой
/// растр, а с ним и пустая страница (вся папка `background-size/vector`).
/// Поэтому свои `width`/`height` корня заменяются размером плитки.
fn with_viewport(markup: &str, tile: (f32, f32)) -> String {
    let Some(open) = markup.find("<svg") else {
        return markup.to_string();
    };
    let Some(close) = markup[open..].find('>').map(|e| open + e) else {
        return markup.to_string();
    };
    let mut head = markup[open + 4..close].to_string();
    for name in ["width", "height"] {
        while let Some(at) = head.find(&format!("{name}=")) {
            let rest = &head[at + name.len() + 1..];
            let Some(quote) = rest.chars().next() else { break };
            let Some(end) = rest[1..].find(quote) else { break };
            head.replace_range(at..at + name.len() + 2 + end + 1, "");
        }
    }
    format!(
        "{}<svg width=\"{}\" height=\"{}\"{head}>{}",
        &markup[..open],
        tile.0,
        tile.1,
        &markup[close + 1..]
    )
}

/// Градиент как источник картинки: считается по своей формуле в растр 64×64.
///
/// Для рамки-картинки соотношение сторон источника роли не играет — куски всё
/// равно растягиваются под свои места, поэтому мелкого растра достаточно.
/// Линейный и конический считаются честно; радиальный отдаёт осевой ход
/// цвета — девятке рамки радиальной решётки и не нужно.
fn gradient_image(src: &str) -> Option<Source> {
    const N: u32 = 64;
    enum Mode {
        /// Ход цвета вдоль оси под углом.
        Axis { dx: f32, dy: f32 },
        /// Оборот вокруг середины от верха по часовой (css-images-4 §2.3).
        Sweep { from: f32 },
    }
    let (mode, stops) = if let Some(inner) = src
        .strip_prefix("conic-gradient(")
        .and_then(|t| t.strip_suffix(')'))
    {
        let parts = crate::css::split_args(inner);
        let mut idx = 0usize;
        let mut from = 0.0f32;
        if let Some(first) = parts.first().map(|f| f.trim())
            && (first.starts_with("from ") || first.starts_with("at "))
        {
            idx = 1;
            if let Some(a) = first.strip_prefix("from ") {
                from = angle_fraction(a.split_whitespace().next().unwrap_or("")).unwrap_or(0.0);
            }
        }
        // Стоп: цвет и до двух позиций-углов; две позиции — это ДВА стопа
        // одного цвета. Цвет с запятыми внутри (`rgba(…)`) остаётся одним
        // словом только при резке вне скобок.
        let mut raw: Vec<(crate::value::Color, Option<f32>)> = vec![];
        for part in &parts[idx..] {
            let words = crate::computed::split_outside_parens(part);
            let Some(colour) = words.first().and_then(|w| crate::value::Color::parse(w)) else {
                continue;
            };
            let angles: Vec<f32> = words[1..].iter().filter_map(|w| angle_fraction(w)).collect();
            if angles.is_empty() {
                raw.push((colour, None));
            }
            for a in angles {
                raw.push((colour, Some(a)));
            }
        }
        if raw.is_empty() {
            return None;
        }
        (Mode::Sweep { from }, place_stops(raw))
    } else {
        let g = crate::computed::parse_gradient(src)?;
        let angle = g.angle_deg.to_radians();
        (Mode::Axis { dx: angle.sin(), dy: -angle.cos() }, g.stops.clone())
    };

    let mut bytes = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 / (N - 1) as f32 - 0.5, y as f32 / (N - 1) as f32 - 0.5);
            let t = match mode {
                Mode::Axis { dx, dy } => (fx * dx + fy * dy + 0.5).clamp(0.0, 1.0),
                Mode::Sweep { from } => {
                    let turn = fx.atan2(-fy) / std::f32::consts::TAU;
                    (turn - from).rem_euclid(1.0)
                }
            };
            let colour = colour_at(&stops, t);
            // Порядок BGRA, премультипликация по прозрачности.
            bytes.push((colour.b * colour.a * 255.0) as u8);
            bytes.push((colour.g * colour.a * 255.0) as u8);
            bytes.push((colour.r * colour.a * 255.0) as u8);
            bytes.push((colour.a * 255.0) as u8);
        }
    }
    let image = gpui::bgra_bytes_to_image(N, N, bytes)?;
    Some(Source::Raster(image))
}

/// Угол позиции стопа в долях оборота: `90deg`, `25%`, `0.25turn`, голый `0`.
fn angle_fraction(token: &str) -> Option<f32> {
    let token = token.trim();
    if let Some(n) = token.strip_suffix('%') {
        return n.parse::<f32>().ok().map(|v| v / 100.0);
    }
    if let Some(n) = token.strip_suffix("deg") {
        return n.parse::<f32>().ok().map(|v| v / 360.0);
    }
    if let Some(n) = token.strip_suffix("grad") {
        return n.parse::<f32>().ok().map(|v| v / 400.0);
    }
    if let Some(n) = token.strip_suffix("rad") {
        return n.parse::<f32>().ok().map(|v| v / std::f32::consts::TAU);
    }
    if let Some(n) = token.strip_suffix("turn") {
        return n.parse::<f32>().ok();
    }
    // Ноль без единицы — законный угол в CSS; прочие голые числа — нет.
    (token == "0").then_some(0.0)
}

/// Расставить позиции стопов по правилам css-images: крайние без позиции — на
/// края, промежуточные — поровну между соседями с позициями, и позиции не
/// убывают.
fn place_stops(
    raw: Vec<(crate::value::Color, Option<f32>)>,
) -> Vec<(crate::value::Color, f32)> {
    let last = raw.len() - 1;
    let mut out: Vec<(crate::value::Color, f32)> = Vec::with_capacity(raw.len());
    let mut floor = 0.0f32;
    for (i, (colour, pos)) in raw.iter().enumerate() {
        let at = match pos {
            Some(v) => v.max(floor),
            None if i == 0 => 0.0,
            None if i == last => 1.0f32.max(floor),
            None => {
                // Доля до следующего стопа с позицией (или до конца).
                let (mut next, mut steps) = (1.0f32, (last - i + 1) as f32);
                for (j, (_, p)) in raw.iter().enumerate().skip(i + 1) {
                    if let Some(v) = p {
                        next = v.max(floor);
                        steps = (j - i + 1) as f32;
                        break;
                    }
                }
                floor + (next - floor) / steps
            }
        };
        floor = at;
        out.push((*colour, at));
    }
    out
}

/// Цвет градиента в точке `t` (0..1) по расставленным стопам.
fn colour_at(stops: &[(crate::value::Color, f32)], t: f32) -> crate::value::Color {
    let Some(first) = stops.first() else {
        return crate::value::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    };
    if t <= first.1 {
        return first.0;
    }
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t >= a.1 && t <= b.1 {
            let k = if b.1 > a.1 { (t - a.1) / (b.1 - a.1) } else { 1.0 };
            return crate::value::Color {
                r: a.0.r + (b.0.r - a.0.r) * k,
                g: a.0.g + (b.0.g - a.0.g) * k,
                b: a.0.b + (b.0.b - a.0.b) * k,
                a: a.0.a + (b.0.a - a.0.a) * k,
            };
        }
    }
    stops.last().map(|s| s.0).unwrap_or(first.0)
}

/// Растр или рисунок — по содержимому файла, а не по расширению: у `data:`-URI
/// расширения нет вовсе.
fn decode(bytes: &[u8]) -> Option<Source> {
    let head = &bytes[..bytes.len().min(512)];
    // Ищем корневой тег, а не начало файла: перед ним стоят и объявление XML,
    // и комментарий с лицензией — с них начинается добрая половина рисунков
    // набора (`background-size/vector/support/*`).
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
    if let Some(profile) = gpui::png_icc_profile(bytes)
        && let Some(fixed) = crate::color_space::apply_icc(&image, &profile)
    {
        return Some(Source::Raster(fixed));
    }
    Some(Source::Raster(image))
}

/// Своя величина рисунка: `width`/`height` корневого тега, иначе `viewBox`.
///
/// Разбирается по тексту, а не деревом: дерево документа рисунка нам не нужно
/// нигде больше, а растеризатору всё равно идёт исходная разметка.
/// Нулевая ось `viewBox`: соотношение вырождено, рисовать нечего.
fn degenerate_viewbox(markup: &str) -> bool {
    let head = match markup.find("<svg") {
        Some(at) => &markup[at..markup[at..].find('>').map(|e| at + e).unwrap_or(markup.len())],
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

fn svg_size(markup: &str) -> Intrinsic {
    let head = match markup.find("<svg") {
        Some(at) => &markup[at..markup[at..].find('>').map(|e| at + e).unwrap_or(markup.len())],
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
        v.trim_end_matches("px").parse().ok().filter(|n: &f32| *n > 0.0)
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

fn read_bytes(src: &str) -> Option<Vec<u8>> {
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
fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
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
fn default_size(i: Intrinsic, area: (f32, f32)) -> (f32, f32) {
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

/// Размер одной плитки в точках по правилам `background-size`.
fn tile_size(i: Intrinsic, box_size: (f32, f32), size: BgSize) -> (f32, f32) {
    let (bw, bh) = box_size;
    // Соотношение для растяжений: своё, иначе — из умолчального размера.
    let auto = default_size(i, box_size);
    let ratio = i.ratio.unwrap_or_else(|| {
        if auto.1 > 0.0 { auto.0 / auto.1 } else { 1.0 }
    });
    match size {
        BgSize::Auto => auto,
        // Без своего соотношения картинка растягивается на место под фон
        // ЦЕЛИКОМ: сохранять нечего (css-images-3 §5.3).
        BgSize::Cover | BgSize::Contain if i.ratio.is_none() => box_size,
        BgSize::Cover | BgSize::Contain => {
            let (iw, ih) = (ratio.max(0.0001), 1.0);
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
        // Заданная одна сторона тянет вторую по соотношению — как в CSS.
        BgSize::Fixed(w, h) => match (len_px(w, bw), len_px(h, bh)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if i.ratio.is_some() || i.w.is_some() => (w, w / ratio),
            (Some(w), None) => (w, auto.1),
            (None, Some(h)) if i.ratio.is_some() || i.h.is_some() => (h * ratio, h),
            (None, Some(h)) => (auto.0, h),
            (None, None) => auto,
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
        Len::Lh(k) => Some(k * 1.2 * 16.0),
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
    c.bg_image.as_ref()?;
    let style = c.clone();
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                paint_area(&style, bounds, window);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// Нарисовать фоновые плитки стиля в ЗАДАННОЙ области.
///
/// Отдельной функцией, а не замыканием слоя: фон РЯДА таблицы рисуется от
/// области ряда, но обрезается прямоугольниками ячеек — вызывающий ставит
/// маску сам и зовёт отрисовку с областью ряда.
pub fn paint_area(c: &Computed, bounds: Bounds<Pixels>, window: &mut gpui::Window) {
    let Some(src) = c.bg_image.clone() else { return };
    let size = c.bg_size;
    let pos = c.bg_pos;
    let repeat = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
    let family = c.font_family.clone().unwrap_or_default();
    let font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, &family, font);
    let border = c.borders();
    // Отступ слоя от ВНУТРЕННЕГО края рамки: слой лежит внутри коробки и
    // меряется именно им, а `background-origin` может требовать другого края
    // (css-backgrounds-3 §3.6). Положительное значение вжимает внутрь.
    let inset = match c.bg_origin {
        Some(crate::computed::BgClip::BorderBox) => [
            -px_of(border.top),
            -px_of(border.right),
            -px_of(border.bottom),
            -px_of(border.left),
        ],
        Some(crate::computed::BgClip::ContentBox) => [
            px_of(c.padding.top),
            px_of(c.padding.right),
            px_of(c.padding.bottom),
            px_of(c.padding.left),
        ],
        _ => [0.0; 4],
    };
    let radius = match c.radius.tl {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let Some(found) = source(&src) else { return };
    // Место под фон: свой край по `background-origin`.
    let bounds = Bounds {
        origin: gpui::point(bounds.origin.x + px(inset[3]), bounds.origin.y + px(inset[0])),
        size: gpui::size(
            bounds.size.width - px(inset[1] + inset[3]),
            bounds.size.height - px(inset[0] + inset[2]),
        ),
    };
    let box_size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let tile = tile_size(found.intrinsic(), box_size, size);
    // Нулевая плитка не рисуется вовсе, а вот МЕЛКАЯ — рисуется: браузер
    // мостит и долями точки. Ограничивается не размер плитки, а их ЧИСЛО.
    if tile.0 <= 0.0 || tile.1 <= 0.0 {
        return;
    }
    // Плитка мельче половины точки неразличима: копии сливаются в сплошную
    // заливку, и мы делаем ровно её — одной растянутой копией. Сливаются
    // только МОСТЯЩИЕСЯ оси (`background-size-near-zero-*`).
    const MERGE: f32 = 0.5;
    let merge = |len: f32, box_len: f32, mode: Tiling| {
        if len < MERGE && mode != Tiling::None {
            box_len
        } else {
            len
        }
    };
    let tile = (
        merge(tile.0, box_size.0, repeat.axis(true)),
        merge(tile.1, box_size.1, repeat.axis(false)),
    );
    // `round` меняет САМ размер плитки, поэтому считается до смещения.
    let tile = (
        rounded(repeat.axis(true), tile.0, box_size.0),
        rounded(repeat.axis(false), tile.1, box_size.1),
    );
    let start = origin(pos, box_size, tile);
    let xs = tiling(repeat.axis(true), start.0, tile.0, box_size.0);
    let ys = tiling(repeat.axis(false), start.1, tile.1, box_size.1);
    let Some(image) = found.raster(tile) else { return };
    let corners = gpui::Corners::all(px(radius));
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        for y in &ys {
            for x in &xs {
                let at = gpui::point(bounds.origin.x + px(*x), bounds.origin.y + px(*y));
                let cell = Bounds {
                    origin: at,
                    size: gpui::size(px(tile.0), px(tile.1)),
                };
                // Промах атласа рисовать нечем — пропускаем плитку молча.
                let _ = window.paint_image(cell, corners, image.clone(), 0, false);
            }
        }
    });
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
    let max = MAX_TILES;
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
            let count = fit.min(max);
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
            let count = ((box_len - first) / tile).ceil().max(1.0).min(max);
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
        let xs = tiling(Tiling::Repeat, 30.0, 20.0, 100.0);
        let first = xs[0];
        assert!(first <= 0.0, "первая плитка начинается не правее коробки");
        assert!(
            first + xs.len() as f32 * 20.0 >= 100.0,
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
