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
mod sampling;
mod sources;
pub use sources::{key, key_exif, source};
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
        // Смесь долей и точек (`of calc(10px + 15%)`): `Len::parse` её
        // отбрасывает, и прежде `?` ронял ВЕСЬ контур — элемент рисовался
        // без обрезки (clip-path-shape-011 и его эталон: 6.83).
        if let Some((p, add)) = crate::value::calc_pct_px(t) {
            return Some(p * side + add);
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
    // Опорная точка КОНТРОЛЬНОЙ точки (css-shapes-2 §2.4.5:
    // `<control-point> = <position> | <coordinate-pair> from [start|end|origin]`).
    // Со словом `from` пара — СМЕЩЕНИЕ от названного якоря; без него точка
    // читается так же, как конец сегмента: у `to` — точка опорной коробки,
    // у `by` — смещение от начала сегмента. Возвращаем ВСЕГДА абсолют,
    // приведение обратно делает сама команда.
    let ctl = |toks: &[&str], i: usize, rel: bool, cur: (f32, f32), end: (f32, f32)| {
        let (dx, dy) = pair(toks.get(i)?, toks.get(i + 1)?)?;
        let anchor = match (toks.get(i + 2).copied(), toks.get(i + 3).copied()) {
            (Some("from"), Some("start")) => Some(cur),
            (Some("from"), Some("end")) => Some(end),
            // Начало опорной коробки — это и есть (0,0) нашей системы.
            (Some("from"), Some("origin")) => Some((0.0, 0.0)),
            _ if rel => Some(cur),
            _ => None,
        };
        Some(match anchor {
            Some((ax, ay)) => (ax + dx, ay + dy),
            None => (dx, dy),
        })
    };
    // Текущая точка контура и начало подконтура (куда возвращает `close`):
    // без них якоря `end` и `origin` посчитать нечем.
    let mut cur = (0.0f32, 0.0f32);
    let mut sub = cur;
    for cmd in args.split(';') {
        // Токены верхнего уровня: `calc(10px + 15%)` не рвётся на три куска.
        let toks: Vec<&str> = split_top(cmd);
        if toks.is_empty() {
            continue;
        }
        match toks[0] {
            "from" | "move" => {
                let base = if toks[0] == "from" { 1 } else { 2 };
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(base)?, toks.get(base + 1)?)?;
                d.push_str(&format!("{}{} {} ", if rel { 'm' } else { 'M' }, x, y));
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                sub = cur;
            }
            "line" => {
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                d.push_str(&format!("{}{} {} ", if rel { 'l' } else { 'L' }, x, y));
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
            }
            "hline" => {
                let rel = toks.get(1) == Some(&"by");
                let x = val(toks.get(2)?, bw)?;
                d.push_str(&format!("{}{} ", if rel { 'h' } else { 'H' }, x));
                cur.0 = if rel { cur.0 + x } else { x };
            }
            "vline" => {
                let rel = toks.get(1) == Some(&"by");
                let y = val(toks.get(2)?, bh)?;
                d.push_str(&format!("{}{} ", if rel { 'v' } else { 'V' }, y));
                cur.1 = if rel { cur.1 + y } else { y };
            }
            "curve" => {
                // curve [to X Y | by dX dY] with C1 [/ C2]; у контрольной
                // точки может стоять свой якорь (`from start|end|origin`).
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                let end = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                // Печатаем в системе САМОЙ команды: у `by` (строчная буква)
                // отсчёт от текущей точки, у `to` — от начала коробки.
                // Без слова `from` это возвращает ровно старую пару, поэтому
                // строка для сегодняшних зелёных не меняется.
                let base = if rel { cur } else { (0.0, 0.0) };
                let with_at = toks.iter().position(|t| *t == "with")?;
                let c1 = ctl(&toks[..], with_at + 1, rel, cur, end)?;
                let slash = toks.iter().position(|t| *t == "/");
                if let Some(sl) = slash {
                    let c2 = ctl(&toks[..], sl + 1, rel, cur, end)?;
                    d.push_str(&format!(
                        "{}{} {} {} {} {} {} ",
                        if rel { 'c' } else { 'C' },
                        c1.0 - base.0,
                        c1.1 - base.1,
                        c2.0 - base.0,
                        c2.1 - base.1,
                        x,
                        y
                    ));
                } else {
                    d.push_str(&format!(
                        "{}{} {} {} {} ",
                        if rel { 'q' } else { 'Q' },
                        c1.0 - base.0,
                        c1.1 - base.1,
                        x,
                        y
                    ));
                }
                cur = end;
            }
            "smooth" => {
                // smooth to X Y [with Cx Cy]: с точкой — кубик S, без — T.
                // Якорь контрольной точки — тот же, что у `curve`.
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                let end = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                let base = if rel { cur } else { (0.0, 0.0) };
                if let Some(with_at) = toks.iter().position(|t| *t == "with") {
                    let c = ctl(&toks[..], with_at + 1, rel, cur, end)?;
                    d.push_str(&format!(
                        "{}{} {} {} {} ",
                        if rel { 's' } else { 'S' },
                        c.0 - base.0,
                        c.1 - base.1,
                        x,
                        y
                    ));
                } else {
                    d.push_str(&format!("{}{} {} ", if rel { 't' } else { 'T' }, x, y));
                }
                cur = end;
            }
            "arc" => {
                // arc to X Y of RX [RY] [cw|ccw] [large|small] [rotate A]
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                let of_at = toks.iter().position(|t| *t == "of")?;
                // css-shapes-2 `arc`: при ДВУХ значениях доля первого — от
                // ширины, второго — от высоты; ОДНО значение задаёт оба радиуса,
                // и доля меряется от direction-agnostic size
                // sqrt(w² + h²) / sqrt(2), как радиус `circle()`
                // (clip-path-shape-011: 10% на 400x300 = 35.36, а не 40).
                let (rx, ry) = match toks.get(of_at + 2).and_then(|t| val(t, bh)) {
                    Some(ry) => (val(toks.get(of_at + 1)?, bw)?, ry),
                    None => {
                        let diag = ((bw * bw + bh * bh) / 2.0).sqrt();
                        let r = val(toks.get(of_at + 1)?, diag)?;
                        (r, r)
                    }
                };
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
                    rx,
                    ry,
                    rot,
                    large,
                    sweep,
                    x,
                    y
                ));
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
            }
            // `close` возвращает перо в начало подконтура — следующий
            // сегмент считает свой якорь `start` уже оттуда.
            "close" => {
                d.push_str("Z ");
                cur = sub;
            }
            _ => return None,
        }
    }
    (!d.is_empty()).then(|| d.trim_end().to_string())
}

/// Контур `border-shape` состоит только из прямых (polygon, `path()`/`shape()`
/// без кривых)? У таких Blink держит предел митры 1e10 — острые углы уходят
/// шипами; у кривых — 4.0 по умолчанию (`border_shape_painter.cc`).
pub fn shape_is_linear(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.starts_with("polygon(") {
        return true;
    }
    if let Some(d) = raw.strip_prefix("path(") {
        return !d.contains(|c: char| {
            matches!(c, 'C' | 'c' | 'S' | 's' | 'Q' | 'q' | 'T' | 't' | 'A' | 'a')
        });
    }
    if raw.starts_with("shape(") {
        return !(raw.contains("arc") || raw.contains("curve") || raw.contains("smooth"));
    }
    false
}

/// Предел митры обводки `border-shape`: у прямолинейного контура 1000 (эталоны
/// WPT ставят ровно столько; Blink — 1e10), у кривых — 4 по умолчанию SVG.
fn miter_limit(raw: &str) -> f32 {
    if shape_is_linear(raw) { 1000.0 } else { 4.0 }
}

/// Скруглённый прямоугольник → контур `d` дугами; радиусы жмутся одним
/// множителем (css-backgrounds-3 §5.5), как в `rrect_mask`. Вырожденный
/// прямоугольник — пустой контур.
fn rrect_d((x0, y0, w, h): (f32, f32, f32, f32), radii: [(f32, f32); 4]) -> String {
    if w <= 0.0 || h <= 0.0 {
        return String::new();
    }
    let sum = |a: f32, c: f32, side: f32| {
        if a + c > side && a + c > 0.0 {
            side / (a + c)
        } else {
            1.0
        }
    };
    let k = 1.0f32
        .min(sum(radii[0].0, radii[1].0, w))
        .min(sum(radii[3].0, radii[2].0, w))
        .min(sum(radii[0].1, radii[3].1, h))
        .min(sum(radii[1].1, radii[2].1, h));
    let r: Vec<(f32, f32)> = radii.iter().map(|(a, c)| (a * k, c * k)).collect();
    let (x1, y1) = (x0 + w, y0 + h);
    let arc = |rx: f32, ry: f32, x: f32, y: f32| {
        if rx > 0.0 && ry > 0.0 {
            format!("A{rx} {ry} 0 0 1 {x} {y} ")
        } else {
            format!("L{x} {y} ")
        }
    };
    let mut d = format!("M{} {} L{} {} ", x0 + r[0].0, y0, x1 - r[1].0, y0);
    d.push_str(&arc(r[1].0, r[1].1, x1, y0 + r[1].1));
    d.push_str(&format!("L{} {} ", x1, y1 - r[2].1));
    d.push_str(&arc(r[2].0, r[2].1, x1 - r[2].0, y1));
    d.push_str(&format!("L{} {} ", x0 + r[3].0, y1));
    d.push_str(&arc(r[3].0, r[3].1, x0, y1 - r[3].1));
    d.push_str(&format!("L{} {} ", x0, y0 + r[0].1));
    d.push_str(&arc(r[0].0, r[0].1, x0 + r[0].0, y0));
    d.push('Z');
    d
}

/// Контур `<basic-shape>` для ОФСЕТ-ПУТИ (motion-1 §«Equivalent Paths For
/// `<basic-shape>`») в системе опорной коробки w×h с началом в её углу.
///
/// Разборщики те же, что у `shape-outside`/`border-shape` — `rrect_of`
/// (inset/rect/xywh и голое слово-коробка вместе с её радиусами) и
/// `svg_path_of` (polygon, `path()`, `shape()`), — а вот круг и эллипс
/// строит сам вызывающий: у офсет-пути своё начало обхода (самая правая
/// точка) и своё направление (по часовой), а `border_shape_path` пишет их
/// от левой точки против часовой. Прямоугольникам менять нечего:
/// `rrect_d` уже начинает с левого конца верхней стороны и идёт по часовой —
/// ровно как требует §paths.
pub fn motion_shape_d(raw: &str, w: f32, h: f32, radius: [(f32, f32); 4]) -> Option<String> {
    let b = ShapeBox {
        mw: w,
        mh: h,
        rx: 0.0,
        ry: 0.0,
        rw: w,
        rh: h,
        cx: 0.0,
        cy: 0.0,
        cw: w,
        ch: h,
        radius,
        threshold: 0.0,
    };
    if let Some((rect, radii)) = rrect_of(raw, &b) {
        return Some(rrect_d(rect, radii));
    }
    svg_path_of(raw, &b).map(|(d, _)| d)
}

/// Контур базовой фигуры `border-shape` в системе ОПОРНОЙ коробки (начало в
/// её углу, размер rw×rh): `d` для SVG и правило намотки. Circle/ellipse —
/// через `shape_params` (все ключи extent), inset/rect/xywh — `rrect_of`,
/// polygon/path/shape — `svg_path_of` (те же функции, что у `shape-outside`).
/// Пустой `d` — вырожденная фигура (`circle(0%)`, `inset(100px)` шире
/// коробки): не видно ничего, как в Blink
/// (border-shape-collapsed-shape-clips-background).
pub fn border_shape_path(raw: &str, rw: f32, rh: f32) -> Option<(String, &'static str)> {
    let raw = raw.trim();
    if raw.starts_with("circle(") || raw.starts_with("ellipse(") {
        let (cx, cy, rx, ry) = shape_params(raw, rw, rh, 1.0)?;
        if rx <= 0.0 || ry <= 0.0 {
            return Some((String::new(), "nonzero"));
        }
        return Some((
            format!(
                "M{} {} A{rx} {ry} 0 1 0 {} {} A{rx} {ry} 0 1 0 {} {} Z",
                cx - rx,
                cy,
                cx + rx,
                cy,
                cx - rx,
                cy
            ),
            "nonzero",
        ));
    }
    let b = ShapeBox {
        mw: rw,
        mh: rh,
        rx: 0.0,
        ry: 0.0,
        rw,
        rh,
        cx: 0.0,
        cy: 0.0,
        cw: rw,
        ch: rh,
        radius: [(0.0, 0.0); 4],
        threshold: 0.0,
    };
    if let Some((rect, radii)) = rrect_of(raw, &b) {
        return Some((rrect_d(rect, radii), "nonzero"));
    }
    svg_path_of(raw, &b)
}

/// Разметка маски буфера группы под `border-shape`: холст cw×ch — область
/// композита, border-box в нём начинается в (dx, dy) и имеет размер bw×bh.
/// Запись `spec`: `t r b l` края опорной коробки от border-box (наружу
/// положительные), толщина обводки, `t r b l` выноса области, `:` и текст
/// фигуры. Одна фигура — контур ПЛЮС его обводка толщиной рамки (Blink
/// `BorderShapePainter::OuterPath` = shape ∪ stroke: фон и содержимое видны
/// под всем кольцом); две — внешняя фигура как есть (обводка 0).
pub fn border_shape_mask_svg(
    spec: &str,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let (head, raw) = spec.split_once(':')?;
    let v: Vec<f32> = head
        .split_whitespace()
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    if v.len() != 9 {
        return None;
    }
    let (ot, or_, ob, ol, stroke) = (v[0], v[1], v[2], v[3], v[4]);
    let (rw, rh) = ((bw + ol + or_).max(0.0), (bh + ot + ob).max(0.0));
    let (d, rule) = border_shape_path(raw, rw, rh)?;
    let path = if d.is_empty() {
        String::new()
    } else if stroke < 0.0 {
        // Отрицательная обводка — ВНУТРЕННИЙ контур одной фигуры: «фигура
        // минус обводка» (Blink `BorderShapePainter::InnerPath`,
        // `border_shape_painter.cc:105-133`) — чёрная обводка поверх белой
        // заливки съедает полосу внутрь на половину толщины. Так режется
        // переполнение (css-borders-4 §border-shape-overflow-interaction).
        format!(
            r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}"/><path d="{d}" fill="none" stroke="#000000" stroke-width="{}" stroke-miterlimit="{}"/>"##,
            -stroke,
            miter_limit(raw)
        )
    } else {
        let stroke_attr = if stroke > 0.0 {
            format!(
                r##" stroke="#ffffff" stroke-width="{stroke}" stroke-miterlimit="{}""##,
                miter_limit(raw)
            )
        } else {
            String::new()
        };
        format!(r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}"{stroke_attr}/>"##)
    };
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><g transform="translate({} {})">{path}</g></svg>"##,
        dx - ol,
        dy - ot
    ))
}

/// Разметка кольца рамки `border-shape` цветом `colour` (холст и border-box —
/// как у `border_shape_mask_svg`; `outer`/`inner` — текст фигуры и края её
/// опорной коробки от border-box). Одна фигура — SVG-обводка толщиной
/// `stroke` по центру контура (half-border-box: поровну внутрь и наружу);
/// две — «внешняя минус внутренняя» через `<mask>` (не evenodd: внутренняя
/// может выходить за внешнюю, Blink берёт разность путей). `None` — рисовать
/// нечего.
pub fn border_shape_ring_svg(
    outer: (&str, [f32; 4]),
    inner: Option<(&str, [f32; 4])>,
    stroke: f32,
    colour: crate::value::Color,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let rgb = format!(
        "rgb({},{},{})",
        (colour.r * 255.0).round(),
        (colour.g * 255.0).round(),
        (colour.b * 255.0).round()
    );
    let place = |(raw, [t, r, b, l]): (&str, [f32; 4])| -> Option<(String, String, &'static str)> {
        let (d, rule) = border_shape_path(raw, (bw + l + r).max(0.0), (bh + t + b).max(0.0))?;
        Some((format!("translate({} {})", dx - l, dy - t), d, rule))
    };
    let (tr_o, d_o, rule_o) = place(outer)?;
    if d_o.is_empty() {
        return None;
    }
    let body = match inner {
        None => {
            if stroke <= 0.0 {
                return None;
            }
            format!(
                r##"<g transform="{tr_o}"><path d="{d_o}" fill="none" fill-rule="{rule_o}" stroke="{rgb}" stroke-opacity="{}" stroke-width="{stroke}" stroke-miterlimit="{}"/></g>"##,
                colour.a,
                miter_limit(outer.0)
            )
        }
        Some(inner) => {
            let (tr_i, d_i, rule_i) = place(inner)?;
            let hole = if d_i.is_empty() {
                String::new()
            } else {
                format!(
                    r##"<g transform="{tr_i}"><path d="{d_i}" fill="#000000" fill-rule="{rule_i}"/></g>"##
                )
            };
            format!(
                r##"<mask id="ring" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><g transform="{tr_o}"><path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}"/></g>{hole}</mask><rect width="{cw}" height="{ch}" fill="{rgb}" fill-opacity="{}" mask="url(#ring)"/>"##,
                colour.a
            )
        }
    };
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}">{body}</svg>"##
    ))
}

/// Разметка теней `box-shadow` у коробки с `border-shape` (css-borders-4
/// §border-shape-shadow-interaction: «cast as if the shape defined by the
/// outer path were opaque … expanded or contracted by the spread distance,
/// blurred by the blur radius, and then clipped by the border-shape»).
/// Холст, border-box, `outer`/`inner` — как у `border_shape_ring_svg`.
///
/// Наружная тень (Blink `BoxPainterBase::PaintNormalBoxShadow`,
/// `box_painter_base.cc:275-372` ветка `HasBorderShape`): контур внешней
/// фигуры, раздутый на `spread` плюс половина обводки у одной фигуры
/// (`BorderShapePainter::OuterPathWithOffset`, `border_shape_painter.cc:135-195`),
/// сдвинутый на смещение и размытый гауссом σ = blur/2 (`BlurAsSigma`), минус
/// область «фигура ∪ обводка» (`OuterPath`, `:79-103`, клип `kDifference`).
/// Внутренняя (`PaintInsetBoxShadowForBorderShape`, `:410-495`): всё вне
/// внутреннего контура (`InnerPath` = фигура минус обводка, `:105-133`),
/// дыра сжата на `spread` (отрицательный — расширена), сдвинута и размыта;
/// видна только внутри `InnerPath`.
///
/// Покрытие каждой тени собирается в `<mask>` светимостью (белый контур с
/// обводкой — раздутие, чёрный — вырез), а краска кладётся одним
/// прямоугольником сквозь маску: полупрозрачный цвет не красится дважды там,
/// где заливка и обводка перекрываются (border-shape-shadow-semitransparent;
/// Blink для того же берёт объединение путей). Порядок — от последней тени к
/// первой (§box-shadow: «the first shadow is on top»).
pub fn border_shape_shadow_svg(
    outer: (&str, [f32; 4]),
    inner: Option<(&str, [f32; 4])>,
    stroke: f32,
    shadows: &[(crate::computed::Shadow, crate::value::Color)],
    inset: bool,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let place = |(raw, [t, r, b, l]): (&str, [f32; 4])| -> Option<(String, String, &'static str)> {
        let (d, rule) = border_shape_path(raw, (bw + l + r).max(0.0), (bh + t + b).max(0.0))?;
        Some((format!("translate({} {})", dx - l, dy - t), d, rule))
    };
    let (tr_o, d_o, rule_o) = place(outer)?;
    if d_o.is_empty() {
        return None;
    }
    // Одна фигура: обводка по центру контура — «фигура ∪ обводка» наружу и
    // «фигура − обводка» внутрь на половину толщины; две — контуры как есть.
    let half = if inner.is_some() { 0.0 } else { stroke / 2.0 };
    let ml_o = miter_limit(outer.0);
    // Внутренний контур: своя фигура у двух, внешняя у одной.
    let (tr_i, d_i, rule_i, ml_i) = match inner {
        Some(inner) => {
            let (tr, d, rule) = place(inner)?;
            (tr, d, rule, miter_limit(inner.0))
        }
        None => (tr_o.clone(), d_o.clone(), rule_o, ml_o),
    };
    let rgb = |c: crate::value::Color| {
        format!(
            "rgb({},{},{})",
            (c.r * 255.0).round(),
            (c.g * 255.0).round(),
            (c.b * 255.0).round()
        )
    };
    let mut defs = String::new();
    let mut body = String::new();
    for (i, (sh, colour)) in shadows.iter().enumerate().rev() {
        if colour.a <= 0.0 {
            continue;
        }
        let sigma = sh.blur.max(0.0) / 2.0;
        let (f_open, f_close) = if sigma > 0.0 {
            defs.push_str(&format!(
                r##"<filter id="bsf{i}" filterUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><feGaussianBlur stdDeviation="{sigma}"/></filter>"##
            ));
            (format!(r##"<g filter="url(#bsf{i})">"##), "</g>".to_string())
        } else {
            (String::new(), String::new())
        };
        let (ox, oy) = (sh.x, sh.y);
        if !inset {
            // Раздутие на `spread + half`: белая обводка вдвое шире сверх
            // заливки; сжатие (минус) — чёрная обводка поверх заливки.
            let total = sh.spread + half;
            let caster = if total > 0.0 {
                format!(
                    r##"<path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml_o}"/>"##,
                    total * 2.0
                )
            } else if total < 0.0 {
                format!(
                    r##"<path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}"/><path d="{d_o}" fill="none" stroke="#000000" stroke-width="{}" stroke-miterlimit="{ml_o}"/>"##,
                    -total * 2.0
                )
            } else {
                format!(r##"<path d="{d_o}" fill="#ffffff" fill-rule="{rule_o}"/>"##)
            };
            // Вырез «фигура ∪ обводка» — тень не видна под самой рамкой.
            let cut_stroke = if half > 0.0 {
                format!(r##" stroke="#000000" stroke-width="{stroke}" stroke-miterlimit="{ml_o}""##)
            } else {
                String::new()
            };
            defs.push_str(&format!(
                r##"<mask id="bsm{i}" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}">{f_open}<g transform="translate({ox} {oy})"><g transform="{tr_o}">{caster}</g></g>{f_close}<g transform="{tr_o}"><path d="{d_o}" fill="#000000" fill-rule="{rule_o}"{cut_stroke}/></g></mask>"##
            ));
            body.push_str(&format!(
                r##"<rect x="0" y="0" width="{cw}" height="{ch}" fill="{}" fill-opacity="{}" mask="url(#bsm{i})"/>"##,
                rgb(*colour),
                colour.a
            ));
        } else {
            // Дыра = внутренний контур, сжатый на `spread + half`: белая
            // обводка возвращает полосу бросающему; отрицательный разлёт
            // расширяет дыру чёрной обводкой.
            let total = sh.spread + half;
            let hole = if total > 0.0 {
                format!(
                    r##"<path d="{d_i}" fill="#000000" fill-rule="{rule_i}"/><path d="{d_i}" fill="none" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml_i}"/>"##,
                    total * 2.0
                )
            } else if total < 0.0 {
                format!(
                    r##"<path d="{d_i}" fill="#000000" fill-rule="{rule_i}" stroke="#000000" stroke-width="{}" stroke-miterlimit="{ml_i}"/>"##,
                    -total * 2.0
                )
            } else {
                format!(r##"<path d="{d_i}" fill="#000000" fill-rule="{rule_i}"/>"##)
            };
            // Видимость — только внутри `InnerPath` (фигура минус обводка).
            let clip_stroke = if half > 0.0 {
                format!(r##" stroke="#000000" stroke-width="{stroke}" stroke-miterlimit="{ml_i}""##)
            } else {
                String::new()
            };
            defs.push_str(&format!(
                r##"<mask id="bsc{i}" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><g transform="{tr_i}"><path d="{d_i}" fill="#ffffff" fill-rule="{rule_i}"{clip_stroke}/></g></mask><mask id="bsm{i}" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}">{f_open}<g transform="translate({ox} {oy})"><rect x="-10000" y="-10000" width="20000" height="20000" fill="#ffffff"/><g transform="{tr_i}">{hole}</g></g>{f_close}</mask>"##
            ));
            body.push_str(&format!(
                r##"<g mask="url(#bsc{i})"><rect x="0" y="0" width="{cw}" height="{ch}" fill="{}" fill-opacity="{}" mask="url(#bsm{i})"/></g>"##,
                rgb(*colour),
                colour.a
            ));
        }
    }
    if body.is_empty() {
        return None;
    }
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><defs>{defs}</defs>{body}</svg>"##
    ))
}

/// Разметка контура `outline` вокруг `border-shape` (Blink
/// `BorderShapePainter::PaintOutline`, `border_shape_painter.cc:275-334`):
/// полоса между внешним контуром, отодвинутым на `off + width`, и им же,
/// отодвинутым на `off` (у одной фигуры — плюс половина обводки рамки,
/// `OuterPathWithOffset`). Полоса собирается маской: белая обводка вдвое
/// шире внешнего отступа, чёрная — внутреннего, чёрная заливка — нутро;
/// `double` — две полосы по трети толщины (`:313-328`). Холст и border-box —
/// как у `border_shape_ring_svg`.
pub fn border_shape_outline_svg(
    outer: (&str, [f32; 4]),
    single: bool,
    stroke: f32,
    off: f32,
    width: f32,
    double: bool,
    colour: crate::value::Color,
    bw: f32,
    bh: f32,
    dx: f32,
    dy: f32,
    cw: f32,
    ch: f32,
) -> Option<String> {
    if width <= 0.0 {
        return None;
    }
    let (raw, [t, r, b, l]) = outer;
    let (d, rule) = border_shape_path(raw, (bw + l + r).max(0.0), (bh + t + b).max(0.0))?;
    if d.is_empty() {
        return None;
    }
    let tr = format!("translate({} {})", dx - l, dy - t);
    let ml = miter_limit(raw);
    let half = if single { stroke / 2.0 } else { 0.0 };
    // Полоса [r_in, r_out] от контура наружу: отрицательный внутренний
    // радиус (контур вжат внутрь) — чёрная заливка уже съедает нутро,
    // обводка внутрь не нужна.
    let band = |r_in: f32, r_out: f32| -> String {
        let outer_s = if r_out > 0.0 {
            format!(
                r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml}"/>"##,
                r_out * 2.0
            )
        } else {
            format!(r##"<path d="{d}" fill="#ffffff" fill-rule="{rule}"/>"##)
        };
        let inner_s = if r_in > 0.0 {
            format!(
                r##"<path d="{d}" fill="#000000" fill-rule="{rule}" stroke="#000000" stroke-width="{}" stroke-miterlimit="{ml}"/>"##,
                r_in * 2.0
            )
        } else if r_in < 0.0 {
            // Контур внутри фигуры: нутро до него остаётся полосой.
            format!(
                r##"<path d="{d}" fill="#000000" fill-rule="{rule}"/><path d="{d}" fill="none" stroke="#ffffff" stroke-width="{}" stroke-miterlimit="{ml}"/>"##,
                -r_in * 2.0
            )
        } else {
            format!(r##"<path d="{d}" fill="#000000" fill-rule="{rule}"/>"##)
        };
        format!("{outer_s}{inner_s}")
    };
    let r_in = off + half;
    let r_out = off + half + width;
    let body = if double && (width / 3.0).round() >= 1.0 {
        let third = (width / 3.0).round();
        format!("{}{}", band(r_out - third, r_out), band(r_in, r_in + third))
    } else {
        band(r_in, r_out)
    };
    // Полоса `double` внешняя и внутренняя лежат в одной маске: внутренняя
    // чёрная заливка второй полосы не задевает первую — она не выходит за
    // r_in + third < r_out − third.
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><mask id="ol" maskUnits="userSpaceOnUse" x="0" y="0" width="{cw}" height="{ch}"><g transform="{tr}">{body}</g></mask><rect width="{cw}" height="{ch}" fill="rgb({},{},{})" fill-opacity="{}" mask="url(#ol)"/></svg>"##,
        (colour.r * 255.0).round(),
        (colour.g * 255.0).round(),
        (colour.b * 255.0).round(),
        colour.a
    ))
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
/// Запись `rrect(...)` для маски и кольца рамки: восемь радиусов углов
/// (tl tr br bl, по паре rx ry; точки — числом в CSS-точках, доля — `N%`),
/// четыре параметра K формы углов (`corner-shape`, css-borders-4
/// §corner-shaping; `inf`/`-inf` — square/notch, так печатает и читает f32)
/// и, для кольца рамки, `/ t r b l` — толщины сторон в CSS-точках.
/// Доли резолвятся при растре от размера коробки — прежде `Pct` уходил
/// нулём (`side()` в `render::grouped`).
pub fn rrect_spec(c: &Computed, ring: Option<[f32; 4]>) -> String {
    let ell = c.radius_ell.unwrap_or([None; 4]);
    let tok = |l: Option<Len>| match l {
        Some(Len::Px(v)) => format!("{v}"),
        Some(Len::Pct(p)) => format!("{}%", p * 100.0),
        _ => "0".to_string(),
    };
    let radii = [c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl];
    let mut out = String::from("rrect(");
    for (i, r) in radii.iter().enumerate() {
        match ell[i] {
            Some((rx, ry)) => out.push_str(&format!("{rx} {ry} ")),
            None => out.push_str(&format!("{} {} ", tok(*r), tok(*r))),
        }
    }
    let k = c.corner_shape.unwrap_or([1.0; 4]);
    out.push_str(&format!("{} {} {} {}", k[0], k[1], k[2], k[3]));
    if let Some([t, r, b, l]) = ring {
        out.push_str(&format!(" / {t} {r} {b} {l}"));
    }
    out.push(')');
    out
}

/// Порог Blink (`core/style/superellipse.h`, `kHighCurvatureThreshold`):
/// K ≥ 16 — прямой угол (радиус как нулевой), K ≤ −16 — полная выемка.
const CORNER_K_FLAT: f32 = 16.0;

/// Разобранная запись `rrect(...)`: радиусы в физических точках, ужатые
/// одним множителем (§5.5); K по углам; `ring` — толщины рамки t/r/b/l в
/// физических точках (кольцо = контур минус его сжатие на толщину).
struct Rrect {
    corners: [(f32, f32); 4],
    k: [f32; 4],
    ring: Option<[f32; 4]>,
}

fn parse_rrect(args: &str, fw: f32, fh: f32, scale: f32) -> Option<Rrect> {
    let (main, ring) = match args.split_once('/') {
        Some((a, b)) => (a, Some(b)),
        None => (args, None),
    };
    let toks: Vec<&str> = main.split_whitespace().collect();
    if toks.len() != 8 && toks.len() != 12 {
        return None;
    }
    // Доля горизонтального радиуса — от ширины, вертикального — от высоты
    // (css-backgrounds-3 §5.1); точки — CSS-точки, множатся на плотность.
    let mut vals = [0f32; 8];
    for (i, t) in toks[..8].iter().enumerate() {
        vals[i] = match t.strip_suffix('%') {
            Some(p) => p.parse::<f32>().ok()? / 100.0 * if i % 2 == 0 { fw } else { fh },
            None => t.parse::<f32>().ok()? * scale,
        };
    }
    let mut k = [1f32; 4];
    for (i, t) in toks.iter().skip(8).enumerate() {
        k[i] = t.parse::<f32>().ok()?;
    }
    let ring = match ring {
        Some(r) => {
            let v: Vec<f32> = r
                .split_whitespace()
                .filter_map(|t| t.parse::<f32>().ok())
                .map(|v| v * scale)
                .collect();
            if v.len() != 4 {
                return None;
            }
            Some([v[0], v[1], v[2], v[3]])
        }
        None => None,
    };
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
    Some(Rrect {
        corners: [
            (vals[0] * f, vals[1] * f), // tl
            (vals[2] * f, vals[3] * f), // tr
            (vals[4] * f, vals[5] * f), // br
            (vals[6] * f, vals[7] * f), // bl
        ],
        k,
        ring,
    })
}

/// Знаковое расстояние точки до контура (внутри < 0, физические точки) и
/// толщина, на которую в этой точке сжимается контур под кольцо рамки
/// (`hypot(nx·w_x, ny·w_y)` по нормали: при равных толщинах — ровно w, у
/// прямой стороны — её толщина; css-borders-4 §corner-shaping: «nearly
/// consistent distance … or linearly increasing if widths differ»).
///
/// Угол — суперэллипс `|x|^p + |y|^p = 1`, `p = 2^K` (спека и JS-эталон WPT
/// `render-corner-shape.js`; в тексте алгоритма спеки показатель записан с
/// опечаткой). Выпуклый (K ≥ 0) — от внутреннего центра угла; вогнутый
/// (K < 0) — точка внутри коробки, если она СНАРУЖИ суперэллипса того же
/// |K| с центром во внешней вершине (зеркало через диагональ). Расстояние —
/// первого порядка `f/|∇f|`; для K=1 сохранена прежняя формула (точная у
/// окружности), чтобы не сдвинуть ни одного пикселя старых масок.
fn contour_dist(px_: f32, py: f32, fw: f32, fh: f32, r: &Rrect) -> (f32, f32) {
    let ring = r.ring.unwrap_or([0.0; 4]);
    // Внешняя вершина угла, направление внутрь, индексы сторон t/r/b/l по x и y.
    let corners = [
        (0.0, 0.0, 1.0, 1.0, 3usize, 0usize), // tl: лево, верх
        (fw, 0.0, -1.0, 1.0, 1, 0),           // tr: право, верх
        (fw, fh, -1.0, -1.0, 1, 2),           // br: право, низ
        (0.0, fh, 1.0, -1.0, 3, 2),           // bl: лево, низ
    ];
    for (i, &(ox, oy, sx, sy, side_x, side_y)) in corners.iter().enumerate() {
        let (rx, ry) = r.corners[i];
        if rx <= 0.0 || ry <= 0.0 {
            continue;
        }
        let k = r.k[i];
        if k >= CORNER_K_FLAT {
            // square: угол прямой — считается сторонами ниже.
            continue;
        }
        // (ex, ey) — от внешней вершины в долях радиуса.
        let (ex, ey) = (sx * (px_ - ox) / rx, sy * (py - oy) / ry);
        if ex < 0.0 || ey < 0.0 {
            continue;
        }
        let notch = k <= -CORNER_K_FLAT;
        // Выемка режет весь свой квадрант: её внутренние рёбра тоже сглажены.
        let in_region = if notch {
            sx * (px_ - ox) < fw / 2.0 && sy * (py - oy) < fh / 2.0
        } else {
            ex < 1.0 && ey < 1.0
        };
        if !in_region {
            continue;
        }
        let (d, gx, gy) = if notch {
            let (dx, dy) = ((1.0 - ex) * rx, (1.0 - ey) * ry);
            if dx < dy { (dx, 1.0, 0.0) } else { (dy, 0.0, 1.0) }
        } else if k >= 0.0 {
            let (dx, dy) = (1.0 - ex, 1.0 - ey);
            if (k - 1.0).abs() < 1e-3 {
                let dd = (dx * dx + dy * dy).sqrt();
                let grad = ((dx / rx) * (dx / rx) + (dy / ry) * (dy / ry)).sqrt() / dd.max(1e-6);
                ((dd - 1.0) / grad.max(1e-6), dx / rx, dy / ry)
            } else {
                let p = 2f32.powf(k);
                let f = dx.powf(p) + dy.powf(p) - 1.0;
                let (gx, gy) = (p * dx.powf(p - 1.0) / rx, p * dy.powf(p - 1.0) / ry);
                (f / (gx * gx + gy * gy).sqrt().max(1e-6), gx, gy)
            }
        } else {
            let p = 2f32.powf(-k);
            let g = ex.powf(p) + ey.powf(p) - 1.0;
            let (gx, gy) = (p * ex.powf(p - 1.0) / rx, p * ey.powf(p - 1.0) / ry);
            (-g / (gx * gx + gy * gy).sqrt().max(1e-6), gx, gy)
        };
        let n = (gx * gx + gy * gy).sqrt().max(1e-6);
        let erode = ((gx / n) * ring[side_x]).hypot((gy / n) * ring[side_y]);
        return (d, erode);
    }
    // Прямые стороны: расстояние до ближайшей, толщина — её.
    let edges = [-py, px_ - fw, py - fh, -px_]; // t r b l
    let (mut best, mut idx) = (edges[0], 0);
    for (i, e) in edges.iter().enumerate().skip(1) {
        if *e > best {
            best = *e;
            idx = i;
        }
    }
    (best, ring[idx])
}

/// Покрытие точки контуром: заливка либо кольцо (внешний контур минус его
/// сжатие на толщину рамки), 0..1.
fn contour_coverage(px_: f32, py: f32, fw: f32, fh: f32, r: &Rrect) -> f32 {
    let (dist, erode) = contour_dist(px_, py, fw, fh, r);
    let outer = (0.5 - dist).clamp(0.0, 1.0);
    if r.ring.is_none() {
        return outer;
    }
    (outer - (0.5 - dist - erode).clamp(0.0, 1.0)).max(0.0)
}

/// Скруглённый прямоугольник с ЭЛЛИПТИЧЕСКИМИ углами и формой углов:
/// `rrect(tlx tly trx try brx bry blx bly [k k k k] [/ t r b l])` — см.
/// `rrect_spec`. Растеризатор круглит только окружностью — эллиптический
/// `border-radius: H / V` и `corner-shape` уходят альфа-маской.
fn rasterize_rrect(args: &str, w: u32, h: u32, scale: f32) -> Option<Arc<RenderImage>> {
    let (fw, fh) = (w as f32, h as f32);
    let r = parse_rrect(args, fw, fh, scale)?;
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let a = contour_coverage(x as f32 + 0.5, y as f32 + 0.5, fw, fh, &r);
            let v = (a * 255.0) as u8;
            bytes.extend_from_slice(&[v, v, v, v]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

/// Кольцо рамки по контуру в цвете (`corner-shape`): запись `rrect(... / t r
/// b l)`, байты BGRA с ПРЕМУЛЬТИПЛИЦИРОВАННОЙ альфой — так пишет свои растры
/// `color_space.rs` (`b * a * 255`).
pub fn rasterize_ring(
    args: &str,
    w: u32,
    h: u32,
    scale: f32,
    colour: crate::value::Color,
) -> Option<Arc<RenderImage>> {
    let (fw, fh) = (w as f32, h as f32);
    let r = parse_rrect(args, fw, fh, scale)?;
    r.ring?;
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let a = contour_coverage(x as f32 + 0.5, y as f32 + 0.5, fw, fh, &r) * colour.a;
            bytes.extend_from_slice(&[
                (colour.b * a * 255.0).round() as u8,
                (colour.g * a * 255.0).round() as u8,
                (colour.r * a * 255.0).round() as u8,
                (a * 255.0).round() as u8,
            ]);
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
    // Снимается ОДНА закрывающая скобка — своей функции: `trim_end_matches`
    // съедал и скобку последнего `calc(...)` центра
    // (`circle(25% at calc(50% - 10px) calc(50% - 10px))`).
    let rest = rest.strip_suffix(')').unwrap_or(rest);
    // Ключевое слово `at` может стоять ПЕРВЫМ, без радиусов перед ним:
    // `ellipse(at 110px 50%)`. Деление по строке с двумя пробелами такую
    // запись не находило вовсе, и `at` уходило в радиус по X
    // (`shape-outside-ellipse-023`).
    let (rads, pos) = match rest.trim().strip_prefix("at ") {
        Some(p) => ("", Some(p.trim())),
        None => match rest.split_once(" at ") {
            Some((r, p)) => (r.trim(), Some(p.trim())),
            None => (rest.trim(), None),
        },
    };
    // Центр: `at X Y`; доля — от стороны коробки; одиночное слово — сторона.
    let axis = |token: &str, side: f32| -> Option<f32> {
        let t = token.trim();
        match t {
            "center" => Some(side * 0.5),
            "left" | "top" => Some(0.0),
            "right" | "bottom" => Some(side),
            // Смесь долей и точек в центре (`at calc(50% - 10px) …`).
            _ if t.starts_with("calc(") => {
                let (p, add) = crate::value::calc_pct_px(t)?;
                Some(p * side + add * scale)
            }
            _ => match crate::value::Len::parse(t)? {
                crate::value::Len::Px(v) => Some(v * scale),
                crate::value::Len::Pct(p) => Some(p * side),
                _ => None,
            },
        }
    };
    let (cx, cy) = match pos {
        Some(p) => {
            // Токены верхнего уровня: `calc(50% - 10px)` — один токен.
            let toks: Vec<&str> = split_top(p);
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
                    // Одно значение: второе — `center` (css-values-4
                    // §position), но слово `top`/`bottom` — вертикальная ось:
                    // `at top` = `center top` (offset-path-shape-circle-003,
                    // -ellipse-003). Прежде `top` уходило в X и центр вставал
                    // на левую сторону.
                    [a] if vert(a) => ("center", *a),
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
        let dx = if far {
            cx.max(fw - cx)
        } else {
            cx.min(fw - cx)
        };
        let dy = if far {
            cy.max(fh - cy)
        } else {
            cy.min(fh - cy)
        };
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
    // `shape-margin` раздувает фигуру наружу на своё расстояние
    // (css-shapes-1 §2.2): контур обтекания — множество точек не дальше
    // `shape-margin` от исходной фигуры. Раздутие было написано и не
    // подключено — общий растровый путь отдавал профиль как есть.
    dilate(&mut iv, sm, cols, rows);
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

/// Экстенты обтекания вдоль БЛОК-оси вертикального письма.
///
/// В вертикали строки набора — это колонки: блок-ось горизонтальна и идёт
/// от правого края (`vertical-rl`, `sideways-rl`), инлайн-ось вертикальна,
/// а line-left = верх, line-right = низ (css-writing-modes-4 §6.3, таблица
/// logical-to-physical). Поэтому ту же маску формы надо резать СТОЛБЦАМИ, а
/// экстент мерить вдоль физической вертикали. Индекс результата —
/// расстояние от блок-старта margin-box, значение — экстент от своей
/// line-стороны; ровно в этих осях работает `FlowRow::vertical_rl`.
///
/// Раздутие `shape-margin` — тот же диск Минковского (css-shapes-1 §2.2),
/// что и у строчного профиля: `dilate` не знает, какая ось «длинная», ей
/// достаточно поменять местами два размера.
pub fn shape_profile_block(raw: &str, b: &ShapeBox, sm: f32, side: i32) -> Option<Vec<f32>> {
    let rows = b.mh.ceil().max(1.0) as usize;
    let cols = b.mw.ceil().max(1.0) as usize;
    let mask = shape_mask(raw, b, cols, rows)?;
    let t = (b.threshold.clamp(0.0, 1.0) * 255.0) as u8;
    let mut iv: Vec<Option<(i32, i32)>> = (0..cols)
        .map(|x| {
            let first = (0..rows).find(|&y| mask[y * cols + x] > t)?;
            let last = (0..rows).rev().find(|&y| mask[y * cols + x] > t)?;
            Some((first as i32, last as i32 + 1))
        })
        .collect();
    dilate(&mut iv, sm, rows, cols);
    Some(
        (0..cols)
            .rev()
            .map(|x| match iv[x] {
                None => 0.0,
                Some((y1, y2)) => {
                    if side < 0 {
                        (y2 as f32).clamp(0.0, b.mh)
                    } else {
                        b.mh - (y1 as f32).clamp(0.0, b.mh)
                    }
                }
            })
            .collect(),
    )
}

/// Альфа-маска формы в холсте margin-box.
fn shape_mask(raw: &str, b: &ShapeBox, cols: usize, rows: usize) -> Option<Vec<u8>> {
    let raw = raw.trim();
    // Круг и эллипс: вписанный эллипс опорной коробки (css-shapes-1 §3.1.1).
    // Горизонтальный путь сюда с ними не приходит — там они уходят в
    // `FloatShape::Ellipse` раньше; маска нужна вертикали, где форма
    // адресуется по блок-оси и аналитическим эллипсом не выражается.
    // Ветка стоит ПЕРВОЙ намеренно: `rrect_of` узнаёт слово-коробку, и
    // запись `circle(50% at left 40px top 40px) border-box` иначе стала бы
    // прямоугольником.
    if raw.contains("circle(") || raw.contains("ellipse(") {
        let at = raw.find("circle(").or_else(|| raw.find("ellipse("))?;
        let head = &raw[at..];
        let head = match head.find(')') {
            Some(end) => &head[..=end],
            None => head,
        };
        let (cx, cy, rx, ry) = shape_params(head, b.rw, b.rh, 1.0)?;
        let (cx, cy) = (cx + b.rx, cy + b.ry);
        if rx <= 0.0 || ry <= 0.0 {
            return Some(vec![0u8; cols * rows]);
        }
        let mut out = vec![0u8; cols * rows];
        for y in 0..rows {
            let dy = (y as f32 + 0.5 - cy) / ry;
            for x in 0..cols {
                let dx = (x as f32 + 0.5 - cx) / rx;
                if dx * dx + dy * dy <= 1.0 {
                    out[y * cols + x] = 255;
                }
            }
        }
        return Some(out);
    }
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
            // Начало записи ищется от её ИМЕНИ: у `repeating-linear-gradient`
            // перед `-gradient(` дефис, а не пробел, и обрезка по пробелу
            // отдавала растеризатору всю строку целиком.
            let head = raw[..at].rfind(|c: char| c.is_whitespace()).map_or(0, |s| s + 1);
            let start = raw[head..at]
                .rfind("repeating-")
                .map_or(head, |s| head + s);
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
        let inner = raw[at + 6..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 6..]);
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
    // `rect(t r b l)` — края от сторон опорной коробки, `auto` значит край
    // (css-shapes-1 §3.1). Отличается от `inset` тем, что правый и нижний
    // отсчитываются от ЛЕВОГО и ВЕРХНЕГО края, а не внутрь от своих.
    if let Some(at) = raw.find("rect(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        let (sides_s, round_s) = match inner.split_once("round") {
            Some((a, r)) => (a, Some(r)),
            None => (inner, None),
        };
        let v: Vec<&str> = sides_s.split_whitespace().collect();
        let edge = |i: usize, base: f32, dflt: f32| -> f32 {
            match v.get(i).copied() {
                None | Some("auto") => dflt,
                Some(t) => len_px(t, base),
            }
        };
        let (t, r2, bo, l) = (
            edge(0, b.rh, 0.0),
            edge(1, b.rw, b.rw),
            edge(2, b.rh, b.rh),
            edge(3, b.rw, 0.0),
        );
        let rect = (b.rx + l, b.ry + t, (r2 - l).max(0.0), (bo - t).max(0.0));
        let radii = round_s.map(parse_round).unwrap_or([(0.0, 0.0); 4]);
        return Some((rect, radii));
    }
    // `xywh(x y w h)` — угол и размер прямо (css-shapes-1 §3.1).
    if let Some(at) = raw.find("xywh(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        let (sides_s, round_s) = match inner.split_once("round") {
            Some((a, r)) => (a, Some(r)),
            None => (inner, None),
        };
        let v: Vec<&str> = sides_s.split_whitespace().collect();
        let at_i = |i: usize, base: f32| -> f32 {
            v.get(i).map_or(0.0, |t| len_px(t, base))
        };
        let rect = (
            b.rx + at_i(0, b.rw),
            b.ry + at_i(1, b.rh),
            at_i(2, b.rw).max(0.0),
            at_i(3, b.rh).max(0.0),
        );
        let radii = round_s.map(parse_round).unwrap_or([(0.0, 0.0); 4]);
        return Some((rect, radii));
    }
    // Слово-коробка (или пустая/непонятная запись формы НЕ здесь — сюда
    // приходят только распознанные): margin/border/padding/content-box без
    // функции — прямоугольник опорной коробки с её радиусами.
    let word_only = raw.split_whitespace().all(|w| w.ends_with("-box"));
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
        let inner = raw[at + 8..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 8..]);
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
            return Some((
                d,
                if rule == "evenodd" {
                    "evenodd"
                } else {
                    "nonzero"
                },
            ));
        }
    }
    if let Some(at) = raw.find("path(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        {
            // `path( [<fill-rule>,]? <string> )` — css-shapes-1 §3.1:
            // правило намотки стоит ПЕРЕД строкой контура и отделено
            // запятой. Оно не отрезалось, и слово `evenodd` вместе с
            // запятой уезжало в атрибут `d` — контур не разбирался вовсе.
            let mut rule = "nonzero";
            let mut body = inner.trim();
            if let Some(rest) = body.strip_prefix("evenodd") {
                rule = "evenodd";
                body = rest.trim_start().trim_start_matches(',').trim_start();
            } else if let Some(rest) = body.strip_prefix("nonzero") {
                body = rest.trim_start().trim_start_matches(',').trim_start();
            }
            let d = body.trim_matches('"').trim_matches('\'').to_string();
            if d.is_empty() {
                return None;
            }
            return Some((d, rule));
        }
    }
    if let Some(at) = raw.find("shape(") {
        let inner = raw[at + 6..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 6..]);
        {
            let d = shape_to_path(&inner.replace(',', ";"), b.rw, b.rh)?;
            return Some((d, "nonzero"));
        }
    }
    None
}

/// Скруглённый прямоугольник в маску: SDF по угловым эллипсам (та же
/// математика, что `rasterize_rrect`, но с началом и размером).
fn rrect_mask(
    rect: (f32, f32, f32, f32),
    radii: [(f32, f32); 4],
    cols: usize,
    rows: usize,
) -> Vec<u8> {
    let (x0, y0, w, h) = rect;
    let (x1, y1) = (x0 + w, y0 + h);
    // Переполнение радиусов: один множитель от худшей пары смежных
    // (css-backgrounds-3 §5.5).
    let mut k = 1.0f32;
    let sum = |a: f32, c: f32, side: f32| {
        if a + c > side && a + c > 0.0 {
            side / (a + c)
        } else {
            1.0
        }
    };
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
    let dx: Vec<i32> = (0..=r)
        .map(|k| (((r * r - k * k) as f32).sqrt()) as i32)
        .collect();
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
        let Some((x1, x2)) = src[y as usize] else {
            continue;
        };
        let contains =
            |m: i32| -> bool { matches!(src[m as usize], Some((a, b)) if a <= x1 && b >= x2) };
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
            let Some(quote) = rest.chars().next() else {
                break;
            };
            let Some(end) = rest[1..].find(quote) else {
                break;
            };
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
/// `cross-fade(<cf-image>#)` (css-images-4 §2.6, `csswg-drafts/css-images-4/
/// Overview.bs` «cross-fade»): взвешенная сумма картинок в
/// премультиплицированных цветах. Доли: без своей доли картинка делит
/// остаток до 100% поровну с такими же; сумма больше 100% нормируется к 100%;
/// меньше — результат частично прозрачен. Слагаемое — цвет (или
/// `image(<color>)`), градиент или `url()`; растр приводится к размеру плитки
/// ближайшей точкой.
fn rasterize_cross_fade(src: &str, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    let inner = src.strip_prefix("cross-fade(")?;
    let inner = &inner[..inner.rfind(')')?];
    let n = (w * h) as usize;
    let mut items: Vec<(Option<f32>, Vec<u8>)> = vec![];
    for part in crate::css::split_args(inner) {
        let mut pct = None;
        let mut img = None;
        for t in split_top(part.trim()) {
            match t.strip_suffix('%').and_then(|v| v.parse::<f32>().ok()) {
                Some(p) => pct = Some((p / 100.0).clamp(0.0, 1.0)),
                None => img = Some(t),
            }
        }
        let img = img?;
        let colour = crate::value::Color::parse(
            img.strip_prefix("image(").and_then(|t| t.strip_suffix(')')).unwrap_or(img),
        );
        let buf = if let Some(c) = colour {
            let px = [
                (c.b * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.r * 255.0).round() as u8,
                (c.a * 255.0).round() as u8,
            ];
            px.iter().copied().cycle().take(n * 4).collect()
        } else {
            let image = if img.contains("gradient(") {
                rasterize_gradient(img, w, h)?
            } else {
                let url = crate::computed::parse_url(img)?;
                source(&url)?.raster((w as f32, h as f32))?
            };
            let size = image.size(0);
            let (iw, ih) = (size.width.0.max(1) as u32, size.height.0.max(1) as u32);
            let bytes = image.as_bytes(0)?;
            let mut out = Vec::with_capacity(n * 4);
            for y in 0..h {
                let sy = (y * ih / h.max(1)).min(ih - 1);
                for x in 0..w {
                    let sx = (x * iw / w.max(1)).min(iw - 1);
                    let at = ((sy * iw + sx) * 4) as usize;
                    out.extend_from_slice(bytes.get(at..at + 4)?);
                }
            }
            out
        };
        items.push((pct, buf));
    }
    if items.is_empty() {
        return None;
    }
    let given: f32 = items.iter().filter_map(|i| i.0).sum();
    let free = items.iter().filter(|i| i.0.is_none()).count() as f32;
    let weights: Vec<f32> = items
        .iter()
        .map(|i| match i.0 {
            Some(p) if given > 1.0 => p / given,
            Some(p) => p,
            None if given >= 1.0 => 0.0,
            None => (1.0 - given) / free,
        })
        .collect();
    // Точки слоёв и результата — с ПРЯМОЙ альфой (так их отдаёт растеризатор
    // и так их ждёт отрисовка плитки); сумма — в премультиплицированных
    // (css-images-4 §2.6), назад к прямой — делением на итоговую альфу.
    // Без премультипликации полупрозрачный красный 1% тянул смесь к красному
    // (`cross-fade-premultiplied-alpha`), а запись в премультиплицированных
    // темнила итог дважды (`cross-fade-target-alpha`).
    let mut out = vec![0u8; n * 4];
    for p in 0..n {
        let (mut acc, mut alpha) = ([0.0f32; 3], 0.0f32);
        for (it, wt) in items.iter().zip(&weights) {
            let a = it.1[p * 4 + 3] as f32 / 255.0 * wt;
            for ch in 0..3 {
                acc[ch] += it.1[p * 4 + ch] as f32 * a;
            }
            alpha += a;
        }
        for ch in 0..3 {
            out[p * 4 + ch] = if alpha > 0.0 { (acc[ch] / alpha).round().clamp(0.0, 255.0) as u8 } else { 0 };
        }
        out[p * 4 + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    gpui::bgra_bytes_to_image(w, h, out)
}

fn rasterize_gradient(src: &str, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    if crate::computed::parse_image_color(src).is_some() {
        return sources::raster_color(src, w, h);
    }
    if src.starts_with("cross-fade(") {
        return rasterize_cross_fade(src, w, h);
    }
    enum Mode {
        /// Ход цвета вдоль оси под углом.
        Axis { dx: f32, dy: f32 },
        /// Оборот вокруг середины от верха по часовой (css-images-4 §2.3).
        Sweep { from: f32 },
    }
    // Повторение — не отдельная запись, а замощение узора стопов вдоль линии
    // (css-images-3 §3.6): приставка снимается здесь, а доля точки
    // заворачивается по диапазону стопов ниже.
    // Повторение — не отдельная запись, а замощение узора стопов вдоль линии
    // (css-images-3 §3.6): приставка снимается здесь, а доля точки
    // заворачивается по диапазону стопов ниже.
    let repeating = src.starts_with("repeating-");
    let src = src.strip_prefix("repeating-").unwrap_or(src);
    let (mode, stops, space, hue) = if let Some(inner) = src
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
            let angles: Vec<f32> = words[1..]
                .iter()
                .filter_map(|w| angle_fraction(w))
                .collect();
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
        // Разбор конического живёт здесь и суффикс `in <space>` пока не
        // читает: смешение остаётся в гамма-sRGB, как было.
        (
            Mode::Sweep { from },
            place_stops(raw),
            crate::computed::GradSpace::Srgb,
            0u8,
        )
    } else {
        let g = crate::computed::parse_gradient(src)?;
        let angle = g.angle_deg.to_radians();
        let (dx, dy) = (angle.sin(), -angle.cos());
        // Смешанные позиции (точки + доли): точки переводятся в доли ТУТ —
        // длина градиентной линии известна только по размеру плитки
        // (css-images-3 §3.4.1: проекция коробки на ось).
        let stops = if g.stops_raw.iter().any(|(_, _, p)| p.is_some()) {
            let axis = (w as f32 * dx).abs() + (h as f32 * dy).abs();
            // Доля и точки у ОДНОГО стопа складываются: `calc(100% - 10px)`
            // приехал парой (1.0, −10) — css-values-4 §10.9, доля стопа
            // решается только по длине оси. У стопов из `%` либо из точек
            // вторая половина пуста, и `or` даёт прежний результат.
            let raw: Vec<(crate::value::Color, Option<f32>)> = g
                .stops_raw
                .iter()
                .map(|(c, f, p)| {
                    let px = p.map(|v| if axis > 0.0 { v / axis } else { 0.0 });
                    let at = match (f, px) {
                        (Some(f), Some(px)) => Some(f + px),
                        (f, px) => f.or(px),
                    };
                    (*c, at)
                })
                .collect();
            place_stops(raw)
        } else {
            g.stops.clone()
        };
        (Mode::Axis { dx, dy }, stops, g.space, g.hue)
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
            let t = if repeating { wrap_repeat(t, &stops) } else { t };
            let colour = colour_at(&stops, t, space, hue);
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
pub(crate) fn place_stops(
    raw: Vec<(crate::value::Color, Option<f32>)>,
) -> Vec<(crate::value::Color, f32)> {
    let last = raw.len() - 1;
    let mut out: Vec<(crate::value::Color, f32)> = Vec::with_capacity(raw.len());
    // Зажим снизу — только позицией ПРЕДШЕСТВЕННИКА (css-images-3 §3.5.3):
    // у первого стопа его нет, и отрицательная позиция законна. Нулевой
    // пол сдвигал `calc(-65535000px)` в ноль, и вся коробка красилась
    // первым цветом (`gradient-eval-*`).
    let mut floor = f32::NEG_INFINITY;
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

/// Доля точки в ПОВТОРЯЮЩЕМСЯ градиенте (css-images-3 §3.6): узор стопов
/// повторяется бесконечно в обе стороны со сдвигом на разность позиций
/// последнего и первого стопа. Нулевая разность повторять нечем — спека
/// объявляет такой градиент вырожденным, и точка остаётся как есть.
fn wrap_repeat(t: f32, stops: &[(crate::value::Color, f32)]) -> f32 {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return t;
    };
    let span = last.1 - first.1;
    if span <= 0.0 {
        return t;
    }
    first.1 + (t - first.1).rem_euclid(span)
}


/// Цвет градиента в точке `t` (0..1) по расставленным стопам.
fn colour_at(
    stops: &[(crate::value::Color, f32)],
    t: f32,
    space: crate::computed::GradSpace,
    hue: u8,
) -> crate::value::Color {
    let Some(first) = stops.first() else {
        return crate::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    };
    if t <= first.1 {
        return first.0;
    }
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t >= a.1 && t <= b.1 {
            let k = if b.1 > a.1 {
                (t - a.1) / (b.1 - a.1)
            } else {
                1.0
            };
            // Цвета смешиваются УЖЕ в пространстве интерполяции
            // (css-color-4 §12.2): перевод туда, покомпонентная доля,
            // перевод обратно. Прозрачность живёт отдельно от осей цвета
            // и всегда линейна.
            // Премультипликация (css-images-3 §3.5.3, css-color-4 §12.3):
            // для прямоугольных осей она равна доле `k·a1 / alpha`.
            let alpha = a.0.a + (b.0.a - a.0.a) * k;
            let kc = if alpha > 0.0 { k * b.0.a / alpha } else { k };
            let (r, g, bl) = crate::color_space::mix_in(space, hue, a.0, b.0, kc);
            return crate::value::Color {
                r,
                g,
                b: bl,
                a: alpha,
            };
        }
    }
    stops.last().map(|s| s.0).unwrap_or(first.0)
}

/// Растр или рисунок — по содержимому файла, а не по расширению: у `data:`-URI
/// расширения нет вовсе.
fn decode(bytes: &[u8], orient: bool) -> Option<Source> {
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
fn exif_orientation(bytes: &[u8]) -> Option<u16> {
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
fn orient_image(
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
fn degenerate_viewbox(markup: &str) -> bool {
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

fn svg_size(markup: &str) -> Intrinsic {
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
fn svg_view(bytes: Vec<u8>, id: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    let attr = |tag: &str, name: &str| -> Option<String> {
        let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
        Some(tag[at..at + tag[at..].find('"')?].to_string())
    };
    let mut from = 0;
    let mut view_box = None;
    while let Some(at) = text[from..].find("<view") {
        let start = from + at;
        let Some(end) = text[start..].find('>') else { break };
        let tag = &text[start..start + end];
        if attr(tag, "id").as_deref() == Some(id) {
            view_box = attr(tag, "viewBox");
            break;
        }
        from = start + end;
    }
    let Some(vb) = view_box else {
        return bytes;
    };
    let Some(root) = text.find("<svg") else {
        return bytes;
    };
    let root_end = root + text[root..].find('>').unwrap_or(4);
    let tag = &text[root..root_end];
    let new_tag = match tag.find("viewBox=\"") {
        Some(at) => {
            let v0 = at + 9;
            let v1 = v0 + tag[v0..].find('"').unwrap_or(0);
            format!("{}{}{}", &tag[..v0], vb, &tag[v1..])
        }
        None => format!("<svg viewBox=\"{vb}\"{}", &tag[4..]),
    };
    format!("{}{}{}", &text[..root], new_tag, &text[root_end..]).into_bytes()
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
    let ratio = i
        .ratio
        .unwrap_or_else(|| if auto.1 > 0.0 { auto.0 / auto.1 } else { 1.0 });
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
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => None,
    }
}

/// Смещение первой плитки: проценты считаются от свободного места, как в CSS.
fn origin(pos: BgPos, box_size: (f32, f32), tile: (f32, f32)) -> (f32, f32) {
    let axis = |l: Option<Len>, box_len: f32, tile_len: f32| -> f32 {
        match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(v)) => (box_len - tile_len) * v,
            // `calc(50px + 50%)`: доля — от свободного места, как у чистой
            // доли (css-backgrounds-3 §3.6), точки — как есть. Смесь с
            // третьей природой парой не отдаётся и, как прежде, идёт нулём.
            Some(Len::Calc(i)) => crate::value::calc_get(i)
                .pct_px()
                .map_or(0.0, |(pct, px)| (box_len - tile_len) * pct + px),
            _ => 0.0,
        }
    };
    (
        axis(pos.x, box_size.0, tile.0),
        axis(pos.y, box_size.1, tile.1),
    )
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
    fn rect(&self, clip: Bounds<Pixels>) -> Bounds<Pixels> {
        let (cw, ch) = (f32::from(clip.size.width), f32::from(clip.size.height));
        let w = self.width.unwrap_or(cw - self.left - self.right).max(0.0);
        let h = self.height.unwrap_or(ch - self.top - self.bottom).max(0.0);
        let left_abs = self
            .left_key
            .filter(|_| self.width.is_none())
            .and_then(crate::interact::root_left_prev)
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

    fn left_x(&self, cw: f32, w: f32) -> f32 {
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
pub(crate) fn border_area_fill(c: &Computed) -> Option<crate::value::Color> {
    if c.bg_clip != Some(crate::computed::BgClip::BorderArea) {
        return None;
    }
    flat_fill(c)
}

/// Цвет, которым красится рамка: свой `border-color` поверх заливки
/// `border-area` («ignoring any transparency introduced by border-color»).
pub(crate) fn border_paint(c: &Computed, colour: crate::value::Color) -> crate::value::Color {
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
        gpui::canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
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
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
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
pub(crate) fn over(top: crate::value::Color, base: crate::value::Color) -> crate::value::Color {
    let a = top.a + base.a * (1.0 - top.a);
    if a <= 0.0 {
        return crate::value::Color::default();
    }
    let mix = |t: f32, b: f32| (t * top.a + b * base.a * (1.0 - top.a)) / a;
    crate::value::Color {
        r: mix(top.r, base.r),
        g: mix(top.g, base.g),
        b: mix(top.b, base.b),
        a,
    }
}

/// Единственный цвет растра, если все его точки одинаковы и непрозрачны.
fn flat_colour(src: &str) -> Option<crate::value::Color> {
    let Source::Raster(image) = source(src)? else {
        return None;
    };
    let bytes = image.as_bytes(0)?;
    let first = bytes.get(0..4)?;
    if first[3] != 255 || !bytes.chunks_exact(4).all(|p| p == first) {
        return None;
    }
    // Порядок байтов растра — BGRA (см. `border_image::tests`).
    Some(crate::value::Color {
        r: first[2] as f32 / 255.0,
        g: first[1] as f32 / 255.0,
        b: first[0] as f32 / 255.0,
        a: 1.0,
    })
}

/// Весь фон коробки одним цветом, если он таков: цвет фона, поверх него
/// одноцветный градиент или одноцветный растр, мощённый без зазоров.
/// `None` — фон узорный (или его нет вовсе).
pub(crate) fn flat_fill(c: &Computed) -> Option<crate::value::Color> {
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
pub(crate) fn text_clip_fill(c: &Computed) -> Option<crate::value::Color> {
    if c.bg_clip != Some(crate::computed::BgClip::Text) {
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

/// Нарисовать плитки: `area` задаёт РАЗМЕР и НАЧАЛО ОТСЧЁТА, `canvas` — что
/// именно закрашивается.
///
/// Две области нужны одному фону — КАНВАСУ (CSS 2.1 §14.2): краска «extends to
/// cover the entire canvas», а плитки «are sized and positioned relative to the
/// root element's box as if they were painted for that element alone». У
/// обычной коробки области совпадают, и `None` оставляет прежний путь слово в
/// слово.
pub fn paint_tiles(
    c: &Computed,
    bounds: Bounds<Pixels>,
    canvas: Option<Bounds<Pixels>>,
    window: &mut gpui::Window,
) {
    let Some(src) = c.bg_image.clone() else {
        return;
    };
    let size = c.bg_size;
    let repeat = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
    let family = c.font_family.clone().unwrap_or_default();
    let font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    // Смещение плитки — та же длина, что и всюду: `background-position: 3em`
    // сводится к точкам ЗДЕСЬ, где известны кегль и гарнитура. Ниже по пути
    // непроходная единица читалась как ноль, и плитка вставала в угол
    // (`background-root-019`, `-023`).
    let pos = {
        // Переводятся ТОЛЬКО единицы шрифта: всё прочее (точки, проценты,
        // `calc`) ниже по пути уже понимают, а лишний перевод их портит —
        // ЗАМЕРЕНО: сплошной перевод дал CSS2 4614 -> 4610, вся потеря в
        // семье `border-*-width-applies-to-00*`.
        let to_px = |l: Option<Len>| match l {
            Some(u @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_) | Len::Ic(_) | Len::Lh(_))) => {
                Some(Len::Px(crate::metrics::spacing_px(Some(u), &family, font)))
            }
            other => other,
        };
        crate::computed::BgPos {
            x: to_px(c.bg_pos.x),
            y: to_px(c.bg_pos.y),
        }
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
    // `image-orientation` действует и на ФОНОВУЮ картинку (css-images-3 §5.4,
    // «Applies to: all elements»): развёрнутый и сырой растр — разные
    // картинки с разным природным размером, и ключ обязан их различать
    // (`image-orientation-none-content-images`: четыре `<img>` с
    // `background-image` под `image-orientation: none`).
    let Some(found) = source(&key_exif(&src, c)) else {
        return;
    };
    // Область ПОКРАСКИ (`background-clip`, css-backgrounds-3 §3.7): плитки
    // меряются областью позиционирования, а кладутся по всей краске —
    // border-box по умолчанию заходит под рамку, content-box режется полем
    // (`origin-*`, `background-size-cover-00*`, `background-origin-007`).
    // Слой лежит в padding-box коробки.
    let paint_box = match c.bg_clip {
        Some(crate::computed::BgClip::PaddingBox) | Some(crate::computed::BgClip::Text) => bounds,
        Some(crate::computed::BgClip::ContentBox) => Bounds {
            origin: gpui::point(
                bounds.origin.x + px(px_of(c.padding.left)),
                bounds.origin.y + px(px_of(c.padding.top)),
            ),
            size: gpui::size(
                (bounds.size.width - px(px_of(c.padding.left) + px_of(c.padding.right))).max(px(0.0)),
                (bounds.size.height - px(px_of(c.padding.top) + px_of(c.padding.bottom))).max(px(0.0)),
            ),
        },
        _ => {
            // Порядок краски (CSS 2.2 Прил. E, css-backgrounds-3 §3.7): фон
            // лежит ПОД рамкой, рамка рисуется поверх. Слой плитки — ребёнок
            // коробки и красится ПОСЛЕ её рамки, поэтому под сплошной
            // непрозрачной рамкой область краски ужимается до её внутреннего
            // края: результат тот же, что «под рамкой». Пунктир, `double` и
            // полупрозрачная рамка пропускают фон — там border-box целиком
            // (`background-repeat-001`, `c548-ln-ht-001`, `margin-shorthand-001`).
            let covers = |i: usize| {
                let opaque = c.border_colors[i]
                    .or(c.border_color)
                    .is_none_or(|col| col.a >= 1.0);
                // solid / inset / outset / groove / ridge — сплошная краска.
                matches!(c.border_side_styles[i], Some(3..=6) | Some(9)) && opaque
            };
            let ext = |side: Option<Len>, i: usize| if covers(i) { 0.0 } else { px_of(side) };
            let (t, r, b, l) = (
                ext(border.top, 0),
                ext(border.right, 1),
                ext(border.bottom, 2),
                ext(border.left, 3),
            );
            Bounds {
                origin: gpui::point(bounds.origin.x - px(l), bounds.origin.y - px(t)),
                size: gpui::size(bounds.size.width + px(l + r), bounds.size.height + px(t + b)),
            }
        }
    };
    // Место под фон: свой край по `background-origin`.
    let bounds = Bounds {
        origin: gpui::point(
            bounds.origin.x + px(inset[3]),
            bounds.origin.y + px(inset[0]),
        ),
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
    // Потолок — от БОЛЬШЕЙ стороны коробки: при нулевой высоте места под фон
    // (`height: 0; padding-bottom: 100px; background-origin: content-box`)
    // потолок по своей оси выходил 8 точек, и `cover` 100×50 рисовался
    // полоской 100×8 (`background-size-cover-003`).
    let cap = box_size.0.max(box_size.1).max(1.0) * 8.0;
    let tile = (tile.0.min(cap), tile.1.min(cap));
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
    // Плитки МЕРЯЮТСЯ областью позиционирования, а КЛАДУТСЯ по всей краске:
    // у канваса это весь холст, и полоса `repeat-x` обязана выходить за поля
    // корня (`background-root-016`: «extending … to the left and right edges
    // of the page»).
    let clip = canvas.unwrap_or(paint_box);
    let start = origin(pos, box_size, tile);
    let shift = (
        f32::from(bounds.origin.x - clip.origin.x),
        f32::from(bounds.origin.y - clip.origin.y),
    );
    let span = (f32::from(clip.size.width), f32::from(clip.size.height));
    // `space` раздаёт зазоры внутри ОБЛАСТИ ПОЗИЦИОНИРОВАНИЯ, а не по холсту
    // (css-backgrounds-3 §3.4), поэтому длина ему нужна своя.
    let lay = |mode: Tiling, from: f32, tile: f32, shift: f32, own: f32, all: f32| {
        if mode == Tiling::Space {
            let base: Vec<f32> = tiling(mode, from, tile, own);
            // За областью позиционирования плитки продолжаются с тем же
            // шагом по всей области покраски (css-backgrounds-3 §3.4:
            // «…continue to be repeated at the same spacing»), иначе под
            // рамкой пусто (`background-repeat-space-8`).
            let step = match base.as_slice() {
                [a, b, ..] => b - a,
                _ => tile,
            };
            let mut out: Vec<f32> = Vec::new();
            if step > 0.0 {
                let (first, last) = (base[0], base[base.len() - 1]);
                let mut v = first - step;
                let mut n = 0;
                while v + tile > -shift && n < MAX_TILES as usize {
                    out.push(v);
                    v -= step;
                    n += 1;
                }
                out.reverse();
                out.extend(base.iter().copied());
                let mut v = last + step;
                let mut n = 0;
                while v < all - shift && n < MAX_TILES as usize {
                    out.push(v);
                    v += step;
                    n += 1;
                }
            } else {
                out = base;
            }
            out.into_iter().map(|v| v + shift).collect()
        } else {
            tiling(mode, from + shift, tile, all)
        }
    };
    let mut xs = lay(
        repeat.axis(true),
        start.0,
        tile.0,
        shift.0,
        box_size.0,
        span.0,
    );
    let mut ys = lay(
        repeat.axis(false),
        start.1,
        tile.1,
        shift.1,
        box_size.1,
        span.1,
    );
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
            box_size.0,
            box_size.1,
            tile.0,
            tile.1,
            start.0,
            start.1,
            xs.len(),
            ys.len()
        );
    }
    let Some(image) = found.raster(tile) else {
        return;
    };
    let corners = gpui::Corners::all(px(radius));
    // Собственная обрезка коробки (`overflow` ≠ visible) режет по её
    // padding-box, а фон по `background-clip` живёт до border-box: маска
    // раздвигается на рамку — ровно на то, что коробка отняла у себя сама
    // (`attachment-local-clipping-image-*`).
    let clips_self = matches!(c.overflow_x, Some(o) if o != crate::computed::Overflow::Visible)
        || matches!(c.overflow_y, Some(o) if o != crate::computed::Overflow::Visible);
    let outer = if clips_self {
        let cur = window.content_mask().bounds;
        Bounds {
            origin: gpui::point(
                cur.origin.x - px(px_of(border.left)),
                cur.origin.y - px(px_of(border.top)),
            ),
            size: gpui::size(
                cur.size.width + px(px_of(border.left) + px_of(border.right)),
                cur.size.height + px(px_of(border.top) + px_of(border.bottom)),
            ),
        }
    } else {
        window.content_mask().bounds
    };
    window.with_content_mask_replaced(gpui::ContentMask { bounds: outer }, |window| {
    window.with_content_mask(Some(gpui::ContentMask { bounds: clip }), |window| {
        for y in &ys {
            for x in &xs {
                let at = gpui::point(clip.origin.x + px(*x), clip.origin.y + px(*y));
                let cell = Bounds {
                    origin: at,
                    size: gpui::size(px(tile.0), px(tile.1)),
                };
                sampling::paint_tile(window, cell, corners, image.clone(), &found);
            }
        }
    });
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
            (0..count as u32).map(|i| i as f32 * (tile + gap)).collect()
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
