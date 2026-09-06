//! `offset-path`/`offset-distance`/`offset-rotate` (motion-1).
//!
//! Offset-трансформ по §«Calculating The Offset Transform» — это сдвиг,
//! совмещающий точку привязки коробки с точкой пути, и поворот. Отдельного
//! конвейера под него нет: строка `transform` синтезируется и уходит в тот
//! же разборщик (`Computed::apply_one`), а матрицы складываются в порядке
//! слоения — сначала offset, поверх него авторский `transform`.

use crate::computed::Computed;
use crate::value::Len;

/// Ломаная контура: точки в системе координат коробки и признак замыкания.
struct Poly {
    pts: Vec<(f32, f32)>,
    closed: bool,
}

/// Приставить offset-трансформ к авторскому.
pub fn apply_offset_transform(c: &mut Computed) {
    let Some(css) = offset_transform_css(c) else {
        return;
    };
    let author = c.transform.take();
    c.apply_one("transform", &css);
    if let (Some(off), Some(a)) = (c.transform, author) {
        c.transform = Some(off.then(&a));
    }
}

fn offset_transform_css(c: &Computed) -> Option<String> {
    let raw = c.offset_path.as_deref()?;
    // Луч — не контур, а отрезок из точки отсчёта: ломаную по нему строить
    // нечем, длина зависит от содержащего блока. Отдельная ветка.
    if let Some(args) = raw.strip_prefix("ray(") {
        return ray_css(c, args.trim_end_matches(')'));
    }
    let inner = raw.strip_prefix("path(")?.trim_end_matches(')').trim();
    let d = inner.trim_matches('\'').trim_matches('"');
    let poly = flatten(d)?;
    let total = length(&poly.pts);
    let want = match c.offset_distance {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(p)) => p * total,
        _ => 0.0,
    };
    // Замкнутый путь ходит по кругу, открытый — зажимается.
    let s = if poly.closed && total > 0.0 {
        want.rem_euclid(total)
    } else {
        want.clamp(0.0, total)
    };
    let (p, ang) = sample(&poly.pts, s);
    // `offset-rotate`: `auto` (начальное) — угол касательной, `reverse` —
    // плюс пол-оборота, голый угол — он сам, `auto <angle>` — сумма.
    let rot = match c.offset_rotate.as_deref().map(str::trim) {
        None | Some("auto") => ang,
        Some("reverse") => ang + std::f32::consts::PI,
        Some(other) if other.starts_with("auto") => {
            ang + angle_rad(other.trim_start_matches("auto").trim()).unwrap_or(0.0)
        }
        Some(other) => angle_rad(other).unwrap_or(0.0),
    };
    Some(origin_shift(c, p, rot))
}

/// Сборка строки `translate(P − O) … rotate(A)`.
///
/// Поворот идёт вокруг точки отсчёта преобразования, поэтому вычитание `O`
/// пишется ОТДЕЛЬНЫМИ звеньями: доля своего размера выражается процентным
/// `translate()`, а `calc()` разборщик `transform` не знает.
/// `offset-anchor: auto` (начальное) совпадает с точкой отсчёта — хвостовой
/// сдвиг `O − Anchor` тогда нулевой и не пишется вовсе.
fn origin_shift(c: &Computed, p: (f32, f32), rot: f32) -> String {
    let (ofx, ofy) = c.transform_origin.unwrap_or((0.5, 0.5));
    let (opx, opy) = c.transform_origin_px;
    let mut css = format!("translate({}px, {}px)", p.0, p.1);
    if opx.is_some() || opy.is_some() {
        css.push_str(&format!(
            " translate({}px, {}px)",
            -opx.unwrap_or(0.0),
            -opy.unwrap_or(0.0)
        ));
    }
    if opx.is_none() {
        css.push_str(&format!(" translateX({}%)", -ofx * 100.0));
    }
    if opy.is_none() {
        css.push_str(&format!(" translateY({}%)", -ofy * 100.0));
    }
    css.push_str(&format!(" rotate({}rad)", rot));
    css
}

/// `ray(<angle> && <ray-size>? && contain? && [at <position>]?)`.
///
/// Размер луча (`closest-side` и родня) нужен ТОЛЬКО долевому
/// `offset-distance`: при длине в пикселях он не считается вовсе. Первый
/// заход поддерживает именно пиксельный случай; долевой ждёт прокидывания
/// размера содержащего блока — в стиле его сегодня нет (есть лишь два бита:
/// `computed.rs:962 cb_rtl` и `computed.rs:1376 cb_height_def`).
fn ray_css(c: &Computed, args: &str) -> Option<String> {
    let toks: Vec<&str> = args.split_whitespace().collect();
    let bearing = toks.iter().find_map(|t| angle_rad(t))?;
    // Компасный угол: 0deg смотрит ВВЕРХ, положительные — по часовой.
    // В экранных осях (x вправо, y вниз) это (sin a, -cos a).
    let dir = (bearing.sin(), -bearing.cos());
    let len = match c.offset_distance {
        Some(Len::Px(v)) => v,
        // Доля требует длины луча, а та — опорной коробки: пока пропускаем,
        // чтобы не рисовать заведомо неверное место.
        Some(Len::Pct(_)) => return None,
        _ => 0.0,
    };
    let p = (dir.0 * len, dir.1 * len);
    let rot = match c.offset_rotate.as_deref().map(str::trim) {
        None | Some("auto") => bearing - std::f32::consts::FRAC_PI_2,
        Some("reverse") => bearing + std::f32::consts::FRAC_PI_2,
        Some(other) => angle_rad(other).unwrap_or(0.0),
    };
    Some(origin_shift(c, p, rot))
}

fn angle_rad(t: &str) -> Option<f32> {
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

/// Разложить `d` в ломаную. Кривых в этом кластере нет — эталоны папки
/// заданы прямыми, дуги подключаются вместе с `<basic-shape>`.
fn flatten(d: &str) -> Option<Poly> {
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let mut closed = false;
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    let toks = tokens(d);
    let mut i = 0usize;
    let mut cmd = ' ';
    while i < toks.len() {
        if let Some(ch) = toks[i].chars().next().filter(|c| c.is_ascii_alphabetic()) {
            cmd = ch;
            i += 1;
        }
        let num = |k: usize| -> f32 { toks.get(k).and_then(|t| t.parse().ok()).unwrap_or(0.0) };
        let rel = cmd.is_ascii_lowercase();
        match cmd.to_ascii_lowercase() {
            'm' => {
                let (x, y) = (num(i), num(i + 1));
                cx = if rel { cx + x } else { x };
                cy = if rel { cy + y } else { y };
                sx = cx;
                sy = cy;
                pts.push((cx, cy));
                i += 2;
                // Следующая пара после `m` — уже линия (SVG 2 §PathData).
                cmd = if rel { 'l' } else { 'L' };
            }
            'l' => {
                let (x, y) = (num(i), num(i + 1));
                cx = if rel { cx + x } else { x };
                cy = if rel { cy + y } else { y };
                pts.push((cx, cy));
                i += 2;
            }
            'h' => {
                let x = num(i);
                cx = if rel { cx + x } else { x };
                pts.push((cx, cy));
                i += 1;
            }
            'v' => {
                let y = num(i);
                cy = if rel { cy + y } else { y };
                pts.push((cx, cy));
                i += 1;
            }
            'z' => {
                pts.push((sx, sy));
                cx = sx;
                cy = sy;
                closed = true;
                i += 1;
            }
            _ => break,
        }
    }
    (!pts.is_empty()).then_some(Poly { pts, closed })
}

fn tokens(d: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else if ch == '-' && !cur.is_empty() && !cur.ends_with('e') {
            out.push(std::mem::take(&mut cur));
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn length(p: &[(f32, f32)]) -> f32 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Точка и угол касательной на расстоянии `s` от начала.
///
/// Вырожденный путь (`m 120 0 h 0 v 0 z`) даёт одну точку и нулевой угол —
/// именно этого ждут `offset-distance-004..006`.
fn sample(p: &[(f32, f32)], s: f32) -> ((f32, f32), f32) {
    if p.len() < 2 {
        return (*p.first().unwrap_or(&(0.0, 0.0)), 0.0);
    }
    let mut left = s;
    for w in p.windows(2) {
        let seg = dist(w[0], w[1]);
        if seg <= 0.0 {
            continue;
        }
        if left <= seg {
            let t = left / seg;
            let pt = (
                w[0].0 + (w[1].0 - w[0].0) * t,
                w[0].1 + (w[1].1 - w[0].1) * t,
            );
            return (pt, (w[1].1 - w[0].1).atan2(w[1].0 - w[0].0));
        }
        left -= seg;
    }
    let last = p[p.len() - 1];
    let prev = p[p.len() - 2];
    (last, (last.1 - prev.1).atan2(last.0 - prev.0))
}
