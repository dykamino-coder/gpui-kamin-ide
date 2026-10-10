//! Геометрия CSS ray для motion path.

use super::Cb;
use crate::animation::motion::anchors::origin_shift;
use crate::animation::motion::anchors::position_in;
use crate::animation::motion::anchors::rotation;
use crate::animation::motion::anchors::start_of;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// `ray(<angle> && <ray-size>? && contain? && [at <position>]?)` (§ray()).
///
/// Луч — не контур, а отрезок из начала: ломаной по нему не строят, длину
/// задаёт `<ray-size>` от содержащего блока. Без опорной коробки (`rb == None`)
/// работает только пиксельный `offset-distance` — как и раньше.
pub(super) fn ray_css(
    c: &Computed,
    args: &str,
    cb: Option<&Cb>,
    rb: Option<(f32, f32, f32, f32)>,
) -> Option<String> {
    let toks: Vec<&str> = args.split_whitespace().collect();
    let bearing = toks.iter().find_map(|t| angle_rad(t))?;
    // Компасный угол: 0deg смотрит ВВЕРХ, положительные — по часовой
    // («`<angle>` values are interpreted as bearing angles, with 0deg pointing
    // up and positive angles representing clockwise rotation»). В экранных
    // осях (x вправо, y вниз) это (sin a, -cos a).
    let dir = (bearing.sin(), -bearing.cos());
    // Начало: `at <position>` сильнее всего, иначе `offset-position`, иначе
    // центр («If the element doesn't have an offset starting position either,
    // it behaves as `at center`»).
    let start = match (toks.iter().position(|t| *t == "at"), rb) {
        (Some(i), Some(rb)) => position_in(&toks[i + 1..], rb),
        _ => start_of(c, cb, rb),
    };
    let len = match (super::motion_style(c).offset_distance, rb) {
        (Some(Len::Px(v)), _) => v,
        // Доля меряется длиной луча, а та — опорной коробкой: без неё
        // по-прежнему не рисуем ничего, чтобы не встать заведомо не туда.
        (Some(Len::Pct(k)), Some(rb)) => {
            let kind = toks.iter().find_map(|t| ray_size(t)).unwrap_or(0);
            let mut full = ray_len(kind, start, rb, dir);
            // `contain`: «the path's length is reduced by half the width or
            // half the height of the element's border box, whichever is
            // larger, and floored at zero».
            if toks.contains(&"contain") {
                let (sw, sh) = cb.map_or((0.0, 0.0), |g| g.self_size);
                full = (full - sw.max(sh) / 2.0).max(0.0);
            }
            k * full
        }
        (Some(Len::Pct(_)), None) => return None,
        _ => 0.0,
    };
    let own = cb.map_or((0.0, 0.0), |g| g.self_off);
    let p = (start.0 + dir.0 * len - own.0, start.1 + dir.1 * len - own.1);
    // Касательная луча относительно оси X — это компасный угол минус
    // четверть оборота; `reverse`/прибавку читает `rotation`.
    Some(origin_shift(
        c,
        p,
        rotation(c, bearing - std::f32::consts::FRAC_PI_2),
    ))
}

/// `<ray-size> = <radial-extent> | sides`: 0 closest-side, 1 closest-corner,
/// 2 farthest-side, 3 farthest-corner, 4 sides. Умолчание — `closest-side`.
pub(super) fn ray_size(t: &str) -> Option<u8> {
    Some(match t {
        "closest-side" => 0,
        "closest-corner" => 1,
        "farthest-side" => 2,
        "farthest-corner" => 3,
        "sides" => 4,
        _ => return None,
    })
}

/// Длина луча от начала `s` до края опорной коробки `rb` (§`<ray-size>`).
///
/// У сторон края считаются ПРЯМЫМИ, продолженными в бесконечность («if the
/// ray's starting point is outside the containing block entirely, the edges of
/// the containing block are considered to extend out to infinity»), поэтому
/// берутся модули расстояний до четырёх линий, а не до отрезков.
pub(super) fn ray_len(kind: u8, s: (f32, f32), rb: (f32, f32, f32, f32), dir: (f32, f32)) -> f32 {
    let (x0, y0, w, h) = rb;
    let (x1, y1) = (x0 + w, y0 + h);
    let dx = [(s.0 - x0).abs(), (x1 - s.0).abs()];
    let dy = [(s.1 - y0).abs(), (y1 - s.1).abs()];
    let corner = |cx: f32, cy: f32| ((cx - s.0).powi(2) + (cy - s.1).powi(2)).sqrt();
    match kind {
        0 => dx[0].min(dx[1]).min(dy[0]).min(dy[1]),
        1 => corner(x0, y0)
            .min(corner(x1, y0))
            .min(corner(x0, y1))
            .min(corner(x1, y1)),
        2 => dx[0].max(dx[1]).max(dy[0]).max(dy[1]),
        3 => corner(x0, y0)
            .max(corner(x1, y0))
            .max(corner(x0, y1))
            .max(corner(x1, y1)),
        // `sides` — до пересечения ЛУЧА с границей блока; начало на границе
        // или снаружи даёт ноль (§`<ray-size>`/sides).
        _ => {
            if s.0 < x0 || s.0 > x1 || s.1 < y0 || s.1 > y1 {
                return 0.0;
            }
            let t = |num: f32, den: f32| {
                if den.abs() < 1e-6 {
                    return f32::INFINITY;
                }
                let t = num / den;
                // Сторона, на которой начало уже стоит (t = 0), не пересечение:
                // из угла (0,0) под 90deg луч идёт до ПРАВОЙ стороны
                // (`offset-path-ray-019`: `translateX(100px)` и `200px`).
                // Луч наружу со стороны не находит ни одного t > 0 — длина 0,
                // как и велит §ray() для начала на границе.
                if t > 1e-4 { t } else { f32::INFINITY }
            };
            let hit = t(x0 - s.0, dir.0)
                .min(t(x1 - s.0, dir.0))
                .min(t(y0 - s.1, dir.1))
                .min(t(y1 - s.1, dir.1));
            if hit.is_finite() { hit } else { 0.0 }
        }
    }
}

pub(super) fn angle_rad(t: &str) -> Option<f32> {
    let v: f32 = t
        .trim_end_matches("deg")
        .trim_end_matches("grad")
        .trim_end_matches("turn")
        .trim_end_matches("rad")
        .parse()
        .ok()?;
    Some(if t.ends_with("grad") {
        v * std::f32::consts::PI / 200.0
    } else if t.ends_with("turn") {
        v * std::f32::consts::TAU
    } else if t.ends_with("rad") {
        v
    } else {
        v.to_radians()
    })
}
