//! Геометрия контура рамки и motion path из CSS-форм.

use super::shape_is_linear;
use crate::paint::background::*;
use crate::style::computed::Computed;

/// Предел митры обводки `border-shape`: у прямолинейного контура 1000 (эталоны
/// WPT ставят ровно столько; Blink — 1e10), у кривых — 4 по умолчанию SVG.
pub(super) fn miter_limit(raw: &str) -> f32 {
    if shape_is_linear(raw) { 1000.0 } else { 4.0 }
}

/// Скруглённый прямоугольник → контур `d` дугами; радиусы жмутся одним
/// множителем (css-backgrounds-3 §5.5), как в `rrect_mask`. Вырожденный
/// прямоугольник — пустой контур.
pub(super) fn rrect_d((x0, y0, w, h): (f32, f32, f32, f32), radii: [(f32, f32); 4]) -> String {
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
    let tok = radius_lengths::token;
    let radii = [c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl];
    let mut out = String::from("rrect(");
    for (i, r) in radii.iter().enumerate() {
        match ell[i] {
            Some((rx, ry)) => out.push_str(&format!("{} {} ", tok(Some(rx)), tok(Some(ry)))),
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
