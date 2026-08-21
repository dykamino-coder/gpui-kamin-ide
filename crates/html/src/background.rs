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
        Source::Gradient { raw } => rasterize_gradient(&raw, 300, 150),
        Source::Shape { raw } => rasterize_shape(&raw, 300, 150, 1.0),
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
    /// Градиент: своей величины НЕТ вовсе (css-images-3 §4.4) — обе оси
    /// берутся от области, а растрируется он точно в размер плитки.
    Gradient { raw: String },
    /// Базовая форма `clip-path` (`circle`/`ellipse`): альфа-маска буфера
    /// группы. Радиусы и центр считаются от размера плитки (= коробки).
    Shape { raw: String },
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
                crate::svg::rasterize(&with_viewport(markup, raster), raster.0, raster.1)
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



/// Перевести `shape()` (css-shapes-2 §2.4) в контур SVG `d`.
///
/// Команды идут через точку с запятой (запятые заменил разбор свойств —
/// по запятым верхнего уровня режутся слои маски). Доли резолвятся здесь:
/// x — от ширины коробки, y — от высоты.
pub fn shape_to_path(args: &str, bw: f32, bh: f32) -> Option<String> {
    let mut d = String::new();
    let val = |t: &str, side: f32| -> Option<f32> {
        // Края коробки словами (hline to right; §2.4.3).
        match t {
            "left" | "top" | "x-start" | "y-start" => return Some(0.0),
            "right" | "bottom" | "x-end" | "y-end" => return Some(side),
            "center" => return Some(side * 0.5),
            _ => {}
        }
        match crate::value::Len::parse(t)? {
            crate::value::Len::Px(v) => Some(v),
            crate::value::Len::Pct(p) => Some(p * side),
            // Шрифтовые единицы — от запасного кегля (16px).
            l => crate::metrics::fallback_len_px(l, "", 16.0),
        }
    };
    // Пара координат из токенов: позиционные слова идут в любом порядке
    // (`from center left` — left это X), горизонтальное слово всегда ось X.
    let pair = |a: &str, b: &str| -> Option<(f32, f32)> {
        let horiz = |t: &str| matches!(t, "left" | "right" | "x-start" | "x-end");
        let vert = |t: &str| matches!(t, "top" | "bottom" | "y-start" | "y-end");
        let (a, b) = if vert(a) || horiz(b) { (b, a) } else { (a, b) };
        Some((val(a, bw)?, val(b, bh)?))
    };
    for cmd in args.split(';') {
        let toks: Vec<&str> = cmd.split_whitespace().collect();
        if toks.is_empty() {
            continue;
        }
        match toks[0] {
            "from" | "move" => {
                let base = if toks[0] == "from" { 1 } else { 2 };
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(base)?, toks.get(base + 1)?)?;
                d.push_str(&format!("{}{} {} ", if rel { 'm' } else { 'M' }, x, y));
            }
            "line" => {
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                d.push_str(&format!("{}{} {} ", if rel { 'l' } else { 'L' }, x, y));
            }
            "hline" => {
                let rel = toks.get(1) == Some(&"by");
                let x = val(toks.get(2)?, bw)?;
                d.push_str(&format!("{}{} ", if rel { 'h' } else { 'H' }, x));
            }
            "vline" => {
                let rel = toks.get(1) == Some(&"by");
                let y = val(toks.get(2)?, bh)?;
                d.push_str(&format!("{}{} ", if rel { 'v' } else { 'V' }, y));
            }
            "curve" => {
                // curve to X Y with C1x C1y [/ C2x C2y]
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                let with_at = toks.iter().position(|t| *t == "with")?;
                let c1 = pair(toks.get(with_at + 1)?, toks.get(with_at + 2)?)?;
                let slash = toks.iter().position(|t| *t == "/");
                if let Some(sl) = slash {
                    let c2 = pair(toks.get(sl + 1)?, toks.get(sl + 2)?)?;
                    d.push_str(&format!(
                        "{}{} {} {} {} {} {} ",
                        if rel { 'c' } else { 'C' },
                        c1.0, c1.1, c2.0, c2.1, x, y
                    ));
                } else {
                    d.push_str(&format!(
                        "{}{} {} {} {} ",
                        if rel { 'q' } else { 'Q' },
                        c1.0, c1.1, x, y
                    ));
                }
            }
            "smooth" => {
                // smooth to X Y [with Cx Cy]: с точкой — кубик S, без — T.
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                if let Some(with_at) = toks.iter().position(|t| *t == "with") {
                    let c = pair(toks.get(with_at + 1)?, toks.get(with_at + 2)?)?;
                    d.push_str(&format!(
                        "{}{} {} {} {} ",
                        if rel { 's' } else { 'S' },
                        c.0, c.1, x, y
                    ));
                } else {
                    d.push_str(&format!("{}{} {} ", if rel { 't' } else { 'T' }, x, y));
                }
            }
            "arc" => {
                // arc to X Y of RX [RY] [cw|ccw] [large|small] [rotate A]
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                let of_at = toks.iter().position(|t| *t == "of")?;
                let rx = val(toks.get(of_at + 1)?, bw)?;
                let ry = toks
                    .get(of_at + 2)
                    .and_then(|t| val(t, bh))
                    .unwrap_or(rx);
                let sweep = if toks.contains(&"cw") { 1 } else { 0 };
                let large = if toks.contains(&"large") { 1 } else { 0 };
                let rot = toks
                    .iter()
                    .position(|t| *t == "rotate")
                    .and_then(|i| toks.get(i + 1))
                    .and_then(|t| t.trim_end_matches("deg").parse::<f32>().ok())
                    .unwrap_or(0.0);
                d.push_str(&format!(
                    "{}{} {} {} {} {} {} {} ",
                    if rel { 'a' } else { 'A' },
                    rx, ry, rot, large, sweep, x, y
                ));
            }
            "close" => d.push_str("Z "),
            _ => return None,
        }
    }
    (!d.is_empty()).then(|| d.trim_end().to_string())
}

/// Слой готового полотна маски: растр плитки и её укладка в device px.
pub struct MaskLayer {
    pub image: Arc<RenderImage>,
    /// Угол и размер плитки в точках полотна.
    pub tile: [f32; 4],
    /// Пооосный запрет мощения.
    pub no_repeat: (bool, bool),
    /// Оператор с накопленным низом: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub op: u8,
    /// Светимость вместо альфы (`mask-mode: luminance`, SVG `<mask>`).
    pub luminance: bool,
}

/// Сложить слои маски в одно полотно (css-masking §7.12, `mask-composite`).
///
/// Слои компонуются С НИЖНЕГО (последнего в списке): оператор каждого
/// действует между ним и стопкой под ним. Выход — альфа-полотно размером с
/// коробку; примитивам выше оно уходит одной плиткой без мощения.
pub fn compose_mask_layers(layers: &[MaskLayer], w: u32, h: u32) -> Option<Arc<RenderImage>> {
    if layers.is_empty() {
        return None;
    }
    let mut acc = vec![0.0f32; (w * h) as usize];
    let mut first = true;
    for layer in layers.iter().rev() {
        let bytes = layer.image.as_bytes(0)?;
        let size = layer.image.size(0);
        let (iw, ih) = (size.width.0.max(1) as usize, size.height.0.max(1) as usize);
        let [tx, ty, tw, th] = layer.tile;
        if tw <= 0.0 || th <= 0.0 {
            continue;
        }
        for y in 0..h as usize {
            for x in 0..w as usize {
                let mut u = (x as f32 + 0.5 - tx) / tw;
                let mut v = (y as f32 + 0.5 - ty) / th;
                let outside = (layer.no_repeat.0 && !(0.0..1.0).contains(&u))
                    || (layer.no_repeat.1 && !(0.0..1.0).contains(&v));
                let a = if outside {
                    0.0
                } else {
                    u = u.rem_euclid(1.0);
                    v = v.rem_euclid(1.0);
                    let px_ = ((u * iw as f32) as usize).min(iw - 1);
                    let py = ((v * ih as f32) as usize).min(ih - 1);
                    let at4 = (py * iw + px_) * 4;
                    if layer.luminance {
                        // Цвет премультиплицирован — взвешенная сумма
                        // сразу равна lum * a (порядок BGRA).
                        (bytes[at4] as f32 * 0.0722
                            + bytes[at4 + 1] as f32 * 0.7152
                            + bytes[at4 + 2] as f32 * 0.2126)
                            / 255.0
                    } else {
                        bytes[at4 + 3] as f32 / 255.0
                    }
                };
                let at = y * w as usize + x;
                let d = acc[at];
                acc[at] = if first {
                    a
                } else {
                    match layer.op {
                        1 => a * (1.0 - d),
                        2 => a * d,
                        3 => a * (1.0 - d) + d * (1.0 - a),
                        _ => a + d * (1.0 - a),
                    }
                };
            }
        }
        first = false;
    }
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for a in acc {
        let v = (a * 255.0) as u8;
        bytes.extend_from_slice(&[v, v, v, v]);
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

/// Альфа-маска базовой формы `clip-path` (css-shapes-1 §3.1).
///
/// `circle(R at X Y)` / `ellipse(RX RY at X Y)`: радиусы — точки, проценты
/// (у круга — от диагонали/√2, у эллипса — от своей оси) или ключевые
/// стороны; центр по умолчанию — середина. Край сглажен по локальному
/// градиенту неявной функции — та же гладкость, что у скругления коробки.
/// Скруглённый прямоугольник с ЭЛЛИПТИЧЕСКИМИ углами: `rrect(tlx tly trx
/// try brx bry blx bly)` в CSS-точках. Растеризатор круглит только
/// окружностью — эллиптический `border-radius: H / V` уходит альфа-маской.
fn rasterize_rrect(args: &str, w: u32, h: u32, scale: f32) -> Option<Arc<RenderImage>> {
    let vals: Vec<f32> = args
        .split_whitespace()
        .filter_map(|t| t.parse::<f32>().ok())
        .map(|v| v * scale)
        .collect();
    if vals.len() != 8 {
        return None;
    }
    let (fw, fh) = (w as f32, h as f32);
    // Переполнение радиусов (css-backgrounds-3 §5.5): все радиусы жмутся
    // ОДНИМ множителем f = min(сторона / сумма смежных радиусов) — а не
    // каждый к половине стороны: у полукруга (`100px 100px 0 0` на 200x100)
    // соседний радиус нулевой, и жать нечего.
    let sum = |a: f32, b: f32| (a + b).max(1e-6);
    let f = (fw / sum(vals[0], vals[2]))
        .min(fw / sum(vals[6], vals[4]))
        .min(fh / sum(vals[1], vals[7]))
        .min(fh / sum(vals[3], vals[5]))
        .min(1.0);
    let corners = [
        (vals[0] * f, vals[1] * f), // tl
        (vals[2] * f, vals[3] * f), // tr
        (vals[4] * f, vals[5] * f), // br
        (vals[6] * f, vals[7] * f), // bl
    ];
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (px_, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Угол пикселя и центр его эллипса; вне угловых прямоугольников
            // расстояние — до ближайшей стороны (знак: внутри отрицателен).
            let (rx, ry, cx, cy) = if px_ < corners[0].0 && py < corners[0].1 {
                (corners[0].0, corners[0].1, corners[0].0, corners[0].1)
            } else if px_ > fw - corners[1].0 && py < corners[1].1 {
                (corners[1].0, corners[1].1, fw - corners[1].0, corners[1].1)
            } else if px_ > fw - corners[2].0 && py > fh - corners[2].1 {
                (corners[2].0, corners[2].1, fw - corners[2].0, fh - corners[2].1)
            } else if px_ < corners[3].0 && py > fh - corners[3].1 {
                (corners[3].0, corners[3].1, corners[3].0, fh - corners[3].1)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };
            let dist = if rx > 0.0 && ry > 0.0 {
                let dx = (px_ - cx) / rx;
                let dy = (py - cy) / ry;
                let d = (dx * dx + dy * dy).sqrt();
                let grad =
                    ((dx / rx) * (dx / rx) + (dy / ry) * (dy / ry)).sqrt() / d.max(1e-6);
                (d - 1.0) / grad.max(1e-6)
            } else {
                (-px_).max(px_ - fw).max(-py).max(py - fh)
            };
            let a = (0.5 - dist).clamp(0.0, 1.0);
            let v = (a * 255.0) as u8;
            bytes.extend_from_slice(&[v, v, v, v]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

/// `scale` — физических точек растра на CSS-точку: точечные величины формы
/// записаны в CSS-точках, а растр может быть плотнее (hidpi).
pub fn rasterize_shape(raw: &str, w: u32, h: u32, scale: f32) -> Option<Arc<RenderImage>> {
    if raw.trim_start().starts_with("rrect(") {
        let rest = raw.split_once('(')?.1;
        return rasterize_rrect(rest.trim_end_matches(')'), w, h, scale);
    }
    let (cx, cy, rx, ry) = shape_params(raw, w as f32, h as f32, scale)?;
    rasterize_ellipse_px(cx, cy, rx, ry, w, h)
}

/// Параметры формы `circle(...)` / `ellipse(...)`: центр и радиусы в
/// точках растра; `scale` переводит точечные величины записи (CSS) в них.
pub fn shape_params(raw: &str, fw: f32, fh: f32, scale: f32) -> Option<(f32, f32, f32, f32)> {
    let (kind, rest) = raw.split_once('(')?;
    let circle = kind.trim().eq_ignore_ascii_case("circle");
    let rest = rest.trim_end_matches(')');
    let (rads, pos) = match rest.split_once(" at ") {
        Some((r, p)) => (r.trim(), Some(p.trim())),
        None => (rest.trim(), None),
    };
    // Центр: `at X Y`; доля — от стороны коробки; одиночное слово — сторона.
    let axis = |token: &str, side: f32| -> Option<f32> {
        let t = token.trim();
        match t {
            "center" => Some(side * 0.5),
            "left" | "top" => Some(0.0),
            "right" | "bottom" => Some(side),
            _ => match crate::value::Len::parse(t)? {
                crate::value::Len::Px(v) => Some(v * scale),
                crate::value::Len::Pct(p) => Some(p * side),
                _ => None,
            },
        }
    };
    let (cx, cy) = match pos {
        Some(p) => {
            let toks: Vec<&str> = p.split_whitespace().collect();
            // Позиционные слова в паре идут в любом порядке: горизонтальное
            // слово — всегда ось X (`at center right`, `at top left`).
            let horiz = |t: &str| matches!(t, "left" | "right");
            let vert = |t: &str| matches!(t, "top" | "bottom");
            // Четырёхзначная запись — пары «край смещение»: `at left 40px
            // top 40px`; от правого/нижнего края смещение зеркалится.
            if toks.len() == 4 {
                let pair = |edge: &str, off: &str, side: f32| -> Option<f32> {
                    let v = axis(off, side)?;
                    Some(match edge {
                        "right" | "bottom" => side - v,
                        _ => v,
                    })
                };
                let horiz_first = horiz(toks[0]);
                let (xe, xo, ye, yo) = if horiz_first {
                    (toks[0], toks[1], toks[2], toks[3])
                } else {
                    (toks[2], toks[3], toks[0], toks[1])
                };
                (
                    pair(xe, xo, fw).unwrap_or(fw * 0.5),
                    pair(ye, yo, fh).unwrap_or(fh * 0.5),
                )
            } else {
                let (tx, ty) = match toks.as_slice() {
                    [a, b] if vert(a) || horiz(b) => (*b, *a),
                    [a, b] => (*a, *b),
                    [a] => (*a, "center"),
                    _ => ("center", "center"),
                };
                (
                    axis(tx, fw).unwrap_or(fw * 0.5),
                    axis(ty, fh).unwrap_or(fh * 0.5),
                )
            }
        }
        None => (fw * 0.5, fh * 0.5),
    };
    // Радиус: точки, доля или ключевая сторона (css-shapes-1 §3.1.1.3):
    // closest/farthest — расстояние от центра до ближайшей/дальней стороны
    // ПО ОСИ (у эллипса — своей), у круга corner — до угла.
    let side_r = |keyword: &str, c: f32, side: f32| -> f32 {
        match keyword {
            "closest-side" => c.abs().min((side - c).abs()),
            "farthest-side" => c.max((side - c).abs()),
            _ => 0.0,
        }
    };
    let corner_r = |far: bool| -> f32 {
        let dx = if far { cx.max(fw - cx) } else { cx.min(fw - cx) };
        let dy = if far { cy.max(fh - cy) } else { cy.min(fh - cy) };
        (dx * dx + dy * dy).sqrt()
    };
    let radius = |token: &str, c: f32, side: f32, pct_base: f32| -> Option<f32> {
        match token {
            "closest-side" => Some(side_r("closest-side", c, side)),
            "farthest-side" => Some(side_r("farthest-side", c, side)),
            "closest-corner" => Some(corner_r(false)),
            "farthest-corner" => Some(corner_r(true)),
            _ => match crate::value::Len::parse(token)? {
                crate::value::Len::Px(v) => Some(v * scale),
                crate::value::Len::Pct(p) => Some(p * pct_base),
                _ => None,
            },
        }
    };
    let diag = ((fw * fw + fh * fh) / 2.0).sqrt();
    let (rx, ry) = if circle {
        let token = rads.split_whitespace().next().unwrap_or("closest-side");
        let r = radius(token, cx, fw, diag)
            .unwrap_or_else(|| side_r("closest-side", cx, fw).min(side_r("closest-side", cy, fh)));
        // Ключевые стороны у круга — по ОБЕИМ осям сразу.
        let r = match token {
            "closest-side" => cx
                .abs()
                .min((fw - cx).abs())
                .min(cy.abs().min((fh - cy).abs())),
            "farthest-side" => cx.max((fw - cx).abs()).max(cy.max((fh - cy).abs())),
            _ => r,
        };
        (r, r)
    } else {
        let mut it = rads.split_whitespace();
        let tx = it.next().unwrap_or("closest-side");
        let ty = it.next().unwrap_or("closest-side");
        // Угловые ключи у эллипса — ЕВКЛИДОВО расстояние до угла, как у
        // круга (clip-path-ellipse-2-ref задаёт rx=√(175²+175²)).
        (
            radius(tx, cx, fw, fw).unwrap_or_else(|| side_r("closest-side", cx, fw)),
            radius(ty, cy, fh, fh).unwrap_or_else(|| side_r("closest-side", cy, fh)),
        )
    };
    Some((cx, cy, rx, ry))
}

/// Альфа-растр эллипса по готовым параметрам в точках растра.
pub fn rasterize_ellipse_px(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    w: u32,
    h: u32,
) -> Option<Arc<RenderImage>> {
    if rx <= 0.0 || ry <= 0.0 {
        // Нулевой радиус — всё скрыто: прозрачная маска.
        return gpui::bgra_bytes_to_image(w, h, vec![0u8; (w * h * 4) as usize]);
    }
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            let d = (dx * dx + dy * dy).sqrt();
            // Расстояние до края в ТОЧКАХ: неявная функция d-1, её градиент
            // по точкам даёт локальный масштаб — без него сглаживание на
            // вытянутом эллипсе было бы шире с одной стороны.
            let grad = ((dx / rx) * (dx / rx) + (dy / ry) * (dy / ry)).sqrt() / d.max(1e-6);
            let px_dist = (d - 1.0) / grad.max(1e-6);
            let a = (0.5 - px_dist).clamp(0.0, 1.0);
            let v = (a * 255.0) as u8;
            bytes.extend_from_slice(&[v, v, v, v]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

/// Разобрать ссылку в источник картинки; результат запоминается.
pub fn source(src: &str) -> Option<Source> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(src)
    {
        return hit.clone();
    }
    let found = if let Some(shape) = src.strip_prefix("shape:") {
        Some(Source::Shape {
            raw: shape.to_string(),
        })
    } else if src.starts_with("linear-gradient(")
        || src.starts_with("radial-gradient(")
        || src.starts_with("conic-gradient(")
    {
        Some(Source::Gradient {
            raw: src.to_string(),
        })
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

// --- Общий путь формы обтекания (css-shapes-1 §3, §shape-margin) ---------
//
// Форма растрируется альфа-маской в холст margin-box (1 пиксель = 1 точка,
// как blink RasterShape), маска сводится к интервалам строк «первый..
// последний непрозрачный», интервалы раздуваются диском shape-margin
// (дилатация Минковского по blink ComputeShapeMarginIntervals) и
// превращаются в экстенты от начала стороны текста. Клип к margin-box
// двойной: холст режет форму, зажим на шаге экстента режет поле
// («a shape can only ever reduce a float area»).

/// Геометрия флоата в системе его margin-box, всё в CSS-точках.
pub struct ShapeBox {
    pub mw: f32,
    pub mh: f32,
    /// Опорная коробка формы.
    pub rx: f32,
    pub ry: f32,
    pub rw: f32,
    pub rh: f32,
    /// Content-box (для картинки/градиента).
    pub cx: f32,
    pub cy: f32,
    pub cw: f32,
    pub ch: f32,
    /// Радиусы опорной коробки (tl,tr,br,bl), эллиптические.
    pub radius: [(f32, f32); 4],
    pub threshold: f32,
}

/// Экстенты обтекания по строкам margin-box; None — форма нераспознана
/// (вызывающий откатывается к прямоугольнику коробки). Пустая форма —
/// нули: «empty float area», НЕ фоллбек.
pub fn shape_profile(raw: &str, b: &ShapeBox, sm: f32, side: i32) -> Option<Vec<f32>> {
    let rows = b.mh.ceil().max(1.0) as usize;
    let cols = b.mw.ceil().max(1.0) as usize;
    let mask = shape_mask(raw, b, cols, rows)?;
    let mut iv = mask_intervals(&mask, cols, rows, b.threshold);
    Some(
        iv.into_iter()
            .map(|slot| match slot {
                None => 0.0,
                Some((x1, x2)) => {
                    if side < 0 {
                        (x2 as f32).clamp(0.0, b.mw)
                    } else {
                        b.mw - (x1 as f32).clamp(0.0, b.mw)
                    }
                }
            })
            .collect(),
    )
}

/// Альфа-маска формы в холсте margin-box.
fn shape_mask(raw: &str, b: &ShapeBox, cols: usize, rows: usize) -> Option<Vec<u8>> {
    let raw = raw.trim();
    // Скруглённый прямоугольник: inset/rect/xywh (+round) и слово-коробка
    // с её радиусами.
    if let Some(rect) = rrect_of(raw, b) {
        return Some(rrect_mask(rect.0, rect.1, cols, rows));
    }
    // Полигон / путь / shape(): готовым SVG-растеризатором — fill-rule и
    // антиалиас даром (как blink ExtractPathData).
    if let Some(d) = svg_path_of(raw, b) {
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cols}" height="{rows}" viewBox="0 0 {cols} {rows}"><path d="{}" fill="#000000" fill-rule="{}"/></svg>"##,
            d.0, d.1
        );
        let img = crate::svg::rasterize(&markup, cols as f32, rows as f32)?;
        let bytes = img.as_bytes(0)?;
        let sz = img.size(0);
        let (iw, ih) = (sz.width.0.max(1) as usize, sz.height.0.max(1) as usize);
        let mut out = vec![0u8; cols * rows];
        for y in 0..rows {
            let sy = (y * ih / rows).min(ih - 1);
            for x in 0..cols {
                let sx = (x * iw / cols).min(iw - 1);
                out[y * cols + x] = bytes[(sy * iw + sx) * 4 + 3];
            }
        }
        return Some(out);
    }
    // Картинка или градиент: альфа в content-box.
    if raw.contains("url(") || raw.contains("-gradient(") {
        let src = if raw.contains("-gradient(") {
            let at = raw.find("-gradient(")?;
            let start = raw[..at]
                .rfind(|c: char| c.is_whitespace())
                .map(|s| s + 1)
                .unwrap_or(0);
            crate::background::source(raw[start..].trim().trim_end_matches(|c| c != ')'))
                .and_then(|s| s.raster((b.cw.max(1.0), b.ch.max(1.0))))
        } else {
            crate::computed::parse_url(raw).and_then(|u| load(&u))
        }?;
        let bytes = src.as_bytes(0)?;
        let sz = src.size(0);
        let (iw, ih) = (sz.width.0.max(1) as usize, sz.height.0.max(1) as usize);
        let mut out = vec![0u8; cols * rows];
        for y in 0..rows {
            let fy = y as f32 - b.cy;
            if fy < 0.0 || fy >= b.ch {
                continue;
            }
            let sy = ((fy / b.ch.max(1.0)) * ih as f32) as usize;
            let sy = sy.min(ih - 1);
            for x in 0..cols {
                let fx = x as f32 - b.cx;
                if fx < 0.0 || fx >= b.cw {
                    continue;
                }
                let sx = ((fx / b.cw.max(1.0)) * iw as f32) as usize;
                out[y * cols + x] = bytes[(sy * iw + sx.min(iw - 1)) * 4 + 3];
            }
        }
        return Some(out);
    }
    None
}

/// inset/rect/xywh/слово-коробка → прямоугольник (x,y,w,h) + радиусы.
fn rrect_of(raw: &str, b: &ShapeBox) -> Option<((f32, f32, f32, f32), [(f32, f32); 4])> {
    let len_px = |t: &str, base: f32| -> f32 {
        match crate::value::Len::parse(t) {
            Some(crate::value::Len::Px(v)) => v,
            Some(crate::value::Len::Pct(k)) => k * base,
            _ => 0.0,
        }
    };
    let parse_round = |tail: &str| -> [(f32, f32); 4] {
        // `round r1 r2 r3 r4 / v1 v2 v3 v4` — как border-radius.
        let (hs, vs) = match tail.split_once('/') {
            Some((a, c)) => (a, c),
            None => (tail, tail),
        };
        let four = |src: &str, base: f32| -> [f32; 4] {
            let v: Vec<f32> = src.split_whitespace().map(|t| len_px(t, base)).collect();
            match v.len() {
                0 => [0.0; 4],
                1 => [v[0]; 4],
                2 => [v[0], v[1], v[0], v[1]],
                3 => [v[0], v[1], v[2], v[1]],
                _ => [v[0], v[1], v[2], v[3]],
            }
        };
        let h = four(hs, b.rw);
        let v = four(vs, b.rh);
        [(h[0], v[0]), (h[1], v[1]), (h[2], v[2]), (h[3], v[3])]
    };
    if let Some(at) = raw.find("inset(") {
        let inner = raw[at + 6..].rsplit_once(')').map(|(a, _)| a).unwrap_or(&raw[at + 6..]);
        {
        let (sides_s, round_s) = match inner.split_once("round") {
            Some((a, r)) => (a, Some(r)),
            None => (inner, None),
        };
        let v: Vec<&str> = sides_s.split_whitespace().collect();
        let side = |i: usize| v.get(i).copied().unwrap_or("0");
        let (t, r, bo, l) = match v.len() {
            1 => (side(0), side(0), side(0), side(0)),
            2 => (side(0), side(1), side(0), side(1)),
            3 => (side(0), side(1), side(2), side(1)),
            _ => (side(0), side(1), side(2), side(3)),
        };
        let (t, r2, bo, l) = (
            len_px(t, b.rh),
            len_px(r, b.rw),
            len_px(bo, b.rh),
            len_px(l, b.rw),
        );
        let rect = (
            b.rx + l,
            b.ry + t,
            (b.rw - l - r2).max(0.0),
            (b.rh - t - bo).max(0.0),
        );
        let radii = round_s.map(parse_round).unwrap_or([(0.0, 0.0); 4]);
        return Some((rect, radii));
        }
    }
    // Слово-коробка (или пустая/непонятная запись формы НЕ здесь — сюда
    // приходят только распознанные): margin/border/padding/content-box без
    // функции — прямоугольник опорной коробки с её радиусами.
    let word_only = raw
        .split_whitespace()
        .all(|w| w.ends_with("-box"));
    if word_only && !raw.is_empty() {
        return Some(((b.rx, b.ry, b.rw, b.rh), b.radius));
    }
    None
}

/// Контур для SVG-растеризатора: polygon / path / shape.
fn svg_path_of(raw: &str, b: &ShapeBox) -> Option<(String, &'static str)> {
    let raw = raw.trim();
    // Функция может идти ПОСЛЕ слова-коробки: `padding-box polygon(...)`.
    if let Some(at) = raw.find("polygon(") {
        let inner = raw[at + 8..].rsplit_once(')').map(|(a, _)| a).unwrap_or(&raw[at + 8..]);
        let inner = inner;
        {
        let mut rule = "nonzero";
        let mut pts_src = inner;
        if let Some(rest) = inner.trim_start().strip_prefix("evenodd") {
            rule = "evenodd";
            pts_src = rest.trim_start().trim_start_matches(',');
        } else if let Some(rest) = inner.trim_start().strip_prefix("nonzero") {
            pts_src = rest.trim_start().trim_start_matches(',');
        }
        let len_px = |t: &str, base: f32| -> f32 {
            match crate::value::Len::parse(t) {
                Some(crate::value::Len::Px(v)) => v,
                Some(crate::value::Len::Pct(k)) => k * base,
                _ => 0.0,
            }
        };
        let mut d = String::new();
        for (i, pair) in pts_src.split(',').enumerate() {
            let mut it = pair.split_whitespace();
            let x = b.rx + len_px(it.next()?, b.rw);
            let y = b.ry + len_px(it.next()?, b.rh);
            d.push_str(if i == 0 { "M" } else { "L" });
            d.push_str(&format!("{x} {y} "));
        }
        if d.is_empty() {
            return None;
        }
        d.push('Z');
        return Some((d, if rule == "evenodd" { "evenodd" } else { "nonzero" }));
        }
    }
    if let Some(at) = raw.find("path(") {
        let inner = raw[at + 5..].rsplit_once(')').map(|(a, _)| a).unwrap_or(&raw[at + 5..]);
        {
        let d = inner
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if d.is_empty() {
            return None;
        }
        return Some((d, "nonzero"));
        }
    }
    if let Some(at) = raw.find("shape(") {
        let inner = raw[at + 6..].rsplit_once(')').map(|(a, _)| a).unwrap_or(&raw[at + 6..]);
        {
        let d = shape_to_path(&inner.replace(',', ";"), b.rw, b.rh)?;
        return Some((d, "nonzero"));
        }
    }
    None
}

/// Скруглённый прямоугольник в маску: SDF по угловым эллипсам (та же
/// математика, что `rasterize_rrect`, но с началом и размером).
fn rrect_mask(rect: (f32, f32, f32, f32), radii: [(f32, f32); 4], cols: usize, rows: usize) -> Vec<u8> {
    let (x0, y0, w, h) = rect;
    let (x1, y1) = (x0 + w, y0 + h);
    // Переполнение радиусов: один множитель от худшей пары смежных
    // (css-backgrounds-3 §5.5).
    let mut k = 1.0f32;
    let sum = |a: f32, c: f32, side: f32| if a + c > side && a + c > 0.0 { side / (a + c) } else { 1.0 };
    k = k.min(sum(radii[0].0, radii[1].0, w));
    k = k.min(sum(radii[3].0, radii[2].0, w));
    k = k.min(sum(radii[0].1, radii[3].1, h));
    k = k.min(sum(radii[1].1, radii[2].1, h));
    let r: Vec<(f32, f32)> = radii.iter().map(|(a, c)| (a * k, c * k)).collect();
    let mut out = vec![0u8; cols * rows];
    for y in 0..rows {
        let fy = y as f32 + 0.5;
        if fy < y0 || fy > y1 {
            continue;
        }
        for x in 0..cols {
            let fx = x as f32 + 0.5;
            if fx < x0 || fx > x1 {
                continue;
            }
            // Углы: попадание в угловую четверть проверяется эллипсом.
            let inside = corner_ok(fx, fy, x0, y0, x1, y1, &r);
            if inside {
                out[y * cols + x] = 255;
            }
        }
    }
    out
}

fn corner_ok(fx: f32, fy: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: &[(f32, f32)]) -> bool {
    let check = |cx: f32, cy: f32, rx: f32, ry: f32| -> bool {
        if rx <= 0.0 || ry <= 0.0 {
            return true;
        }
        let (dx, dy) = ((fx - cx) / rx, (fy - cy) / ry);
        dx * dx + dy * dy <= 1.0
    };
    // tl
    if fx < x0 + r[0].0 && fy < y0 + r[0].1 && !check(x0 + r[0].0, y0 + r[0].1, r[0].0, r[0].1) {
        return false;
    }
    // tr
    if fx > x1 - r[1].0 && fy < y0 + r[1].1 && !check(x1 - r[1].0, y0 + r[1].1, r[1].0, r[1].1) {
        return false;
    }
    // br
    if fx > x1 - r[2].0 && fy > y1 - r[2].1 && !check(x1 - r[2].0, y1 - r[2].1, r[2].0, r[2].1) {
        return false;
    }
    // bl
    if fx < x0 + r[3].0 && fy > y1 - r[3].1 && !check(x0 + r[3].0, y1 - r[3].1, r[3].0, r[3].1) {
        return false;
    }
    true
}

/// Интервалы строк: от первого до последнего пикселя с альфой ВЫШЕ порога
/// (строго; дыры внутри строки заполняются — как blink).
fn mask_intervals(mask: &[u8], cols: usize, rows: usize, thr: f32) -> Vec<Option<(i32, i32)>> {
    let t = (thr.clamp(0.0, 1.0) * 255.0) as u8;
    (0..rows)
        .map(|y| {
            let row = &mask[y * cols..(y + 1) * cols];
            let first = row.iter().position(|a| *a > t)?;
            let last = row.iter().rposition(|a| *a > t)?;
            Some((first as i32, last as i32 + 1))
        })
        .collect()
}

/// Дилатация Минковского диском `sm` (blink ComputeShapeMarginIntervals):
/// каждый интервал раздаётся соседним строкам с сужением по дуге; ранний
/// выход, когда сосед и так шире. Вертикаль жёстко в [0, rows).
fn dilate(iv: &mut [Option<(i32, i32)>], sm: f32, cols: usize, rows: usize) {
    if sm <= 0.0 {
        return;
    }
    let cap = ((cols.max(rows) as f32) * std::f32::consts::SQRT_2) as i32;
    let r = (sm.ceil() as i32).clamp(0, cap.max(1));
    let dx: Vec<i32> = (0..=r).map(|k| (((r * r - k * k) as f32).sqrt()) as i32).collect();
    let src: Vec<Option<(i32, i32)>> = iv.to_vec();
    let top = src.iter().position(|s| s.is_some());
    let bot = src.iter().rposition(|s| s.is_some());
    let (top, bot) = match (top, bot) {
        (Some(a), Some(b)) => (a as i32, b as i32),
        _ => return,
    };
    let unite = |slot: &mut Option<(i32, i32)>, x1: i32, x2: i32| match slot {
        None => *slot = Some((x1, x2)),
        Some((a, b)) => {
            *a = (*a).min(x1);
            *b = (*b).max(x2);
        }
    };
    for y in 0..rows as i32 {
        let Some((x1, x2)) = src[y as usize] else { continue };
        let contains = |m: i32| -> bool {
            matches!(src[m as usize], Some((a, b)) if a <= x1 && b >= x2)
        };
        // вверх
        let y0 = (y - r).max(0);
        let mut my = y - 1;
        while my >= y0 {
            if my > top && contains(my) {
                break;
            }
            let d = dx[(y - my) as usize];
            unite(&mut iv[my as usize], x1 - d, x2 + d);
            my -= 1;
        }
        unite(&mut iv[y as usize], x1 - dx[0], x2 + dx[0]);
        // вниз
        let y1 = (y + r).min(rows as i32 - 1);
        let mut my = y + 1;
        while my <= y1 {
            if my < bot && contains(my) {
                break;
            }
            let d = dx[(my - y) as usize];
            unite(&mut iv[my as usize], x1 - d, x2 + d);
            my += 1;
        }
    }
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
    let mut own = [None::<String>, None::<String>];
    for (i, name) in ["width", "height"].iter().enumerate() {
        while let Some(at) = head.find(&format!("{name}=")) {
            let rest = &head[at + name.len() + 1..];
            let Some(quote) = rest.chars().next() else { break };
            let Some(end) = rest[1..].find(quote) else { break };
            own[i] = Some(rest[1..1 + end].to_string());
            head.replace_range(at..at + name.len() + 2 + end + 1, "");
        }
    }
    // Без viewBox содержимое НЕ растёт под новый вьюпорт: рисунок 50x50 в
    // плитке 100x100 занимал четверть, а маска-плитка выходила с прозрачными
    // полосами (mask-repeat-1, mask-size-cover). Свои размеры рута становятся
    // рамкой просмотра — содержимое масштабируется, как в браузере.
    if !head.contains("viewBox") {
        if let (Some(w), Some(h)) = (&own[0], &own[1]) {
            let plain = |v: &str| v.trim().trim_end_matches("px").parse::<f32>().ok();
            if let (Some(w), Some(h)) = (plain(w), plain(h)) {
                head.push_str(&format!(" viewBox=\"0 0 {w} {h}\""));
            }
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
fn rasterize_gradient(src: &str, w: u32, h: u32) -> Option<Arc<RenderImage>> {
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
        let (dx, dy) = (angle.sin(), -angle.cos());
        // Смешанные позиции (точки + доли): точки переводятся в доли ТУТ —
        // длина градиентной линии известна только по размеру плитки
        // (css-images-3 §3.4.1: проекция коробки на ось).
        let stops = if g.stops_raw.iter().any(|(_, _, p)| p.is_some()) {
            let axis = (w as f32 * dx).abs() + (h as f32 * dy).abs();
            let raw: Vec<(crate::value::Color, Option<f32>)> = g
                .stops_raw
                .iter()
                .map(|(c, f, p)| (*c, f.or(p.map(|v| if axis > 0.0 { v / axis } else { 0.0 }))))
                .collect();
            place_stops(raw)
        } else {
            g.stops.clone()
        };
        (Mode::Axis { dx, dy }, stops)
    };

    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (
                x as f32 / (w.max(2) - 1) as f32 - 0.5,
                y as f32 / (h.max(2) - 1) as f32 - 0.5,
            );
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
    gpui::bgra_bytes_to_image(w, h, bytes)
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

/// Фон КАНВЫ рисунка: `style="background: …"` на корневом `<svg>`.
///
/// Это CSS-свойство замещаемого корня, а не SVG-контент — растеризатор его
/// не рисует, и рисунок из одного фона выходил прозрачным (box-sizing-007).
pub(crate) fn svg_root_background(markup: &str) -> Option<crate::value::Color> {
    let head = match markup.find("<svg") {
        Some(at) => &markup[at..markup[at..].find('>').map(|e| at + e).unwrap_or(markup.len())],
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
        Len::Calc(i) => {
            let s = crate::value::calc_get(i);
            Some(s.px + base * s.pct)
        }
        // Шрифтовые единицы — от запасного кегля, единой точкой.
        l @ (Len::Em(_)
        | Len::EmPx(..)
        | Len::Ch(_)
        | Len::Ic(_)
        | Len::Ex(_)
        | Len::Lh(_)
        | Len::LhPx(..)) => crate::metrics::fallback_len_px(l, "", 16.0),
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
    // Плитка НЕ бывает осмысленно шире считанных коробок: вырожденное
    // соотношение (`viewBox` в миллиарды) давало размер за пределами
    // точности float, и координаты копий разваливались. Видима всё равно
    // только часть в коробке.
    let tile = (
        tile.0.min(box_size.0.max(1.0) * 8.0),
        tile.1.min(box_size.1.max(1.0) * 8.0),
    );
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
    let mut xs = tiling(repeat.axis(true), start.0, tile.0, box_size.0);
    let mut ys = tiling(repeat.axis(false), start.1, tile.1, box_size.1);
    // Общий потолок числа квадов: потолок НА ОСЬ пропускал произведение
    // (плитка 1x1 на вьюпорт = ~480 тысяч квадов — кадр не заканчивался,
    // hidpi-invert-filter-background висел). Плитки ПРОРЕЖИВАЮТСЯ с
    // укрупнением квада: покрытие коробки сохраняется (для одноцветной
    // 1x1 — точно, узор теряет лишь плотность повтора).
    const MAX_QUADS: usize = 4096;
    let mut tile = tile;
    if xs.len() * ys.len() > MAX_QUADS {
        let k = ((xs.len() * ys.len()) as f32 / MAX_QUADS as f32)
            .sqrt()
            .ceil() as usize;
        xs = xs.into_iter().step_by(k).collect();
        ys = ys.into_iter().step_by(k).collect();
        tile = (tile.0 * k as f32, tile.1 * k as f32);
    }
    if std::env::var("HTML_BG").is_ok() {
        eprintln!(
            "BG box=({:.0},{:.0}) tile=({:.0},{:.0}) start=({:.0},{:.0}) xs={} ys={}",
            box_size.0, box_size.1, tile.0, tile.1, start.0, start.1, xs.len(), ys.len()
        );
    }
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
