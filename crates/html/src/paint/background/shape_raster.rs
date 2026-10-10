//! Растр фигур: скруглённые прямоугольники, кольца, эллипсы, параметры фигуры.

mod ellipse;
pub use ellipse::rasterize_ellipse_px;

mod shape_parameters;
pub use shape_parameters::shape_params;

use crate::paint::background::*;
use gpui::RenderImage;
use std::sync::Arc;

/// Порог Blink (`core/style/superellipse.h`, `kHighCurvatureThreshold`):
/// K ≥ 16 — прямой угол (радиус как нулевой), K ≤ −16 — полная выемка.
const CORNER_K_FLAT: f32 = 16.0;

/// Разобранная запись `rrect(...)`: радиусы в физических точках, ужатые
/// одним множителем (§5.5); K по углам; `ring` — толщины рамки t/r/b/l в
/// физических точках (кольцо = контур минус его сжатие на толщину).
struct Rrect {
    pub(crate) corners: [(f32, f32); 4],
    pub(crate) k: [f32; 4],
    pub(crate) ring: Option<[f32; 4]>,
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
        vals[i] = radius_lengths::resolve(t, if i % 2 == 0 { fw } else { fh }, scale)?;
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
            if dx < dy {
                (dx, 1.0, 0.0)
            } else {
                (dy, 0.0, 1.0)
            }
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
            let v = (a * 255.0).round() as u8;
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
    colour: crate::style::values::value::Color,
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
