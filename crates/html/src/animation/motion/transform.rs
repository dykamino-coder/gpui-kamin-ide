//! Расчёт offset transform и выбор геометрии пути.

use super::ellipse_path;
use super::{Cb, SVG_SHAPES, flatten, length, origin_shift, ray_css, rotation, sample, start_of};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn offset_transform_css(c: &Computed, cb: Option<&Cb>) -> Option<String> {
    let raw = super::motion_style(c).offset_path.as_deref()?.trim();
    let (func, kind) = split_coord_box(raw);
    // `path()` — единственная запись в СВОЕЙ системе координат: опорной
    // коробки у неё нет, начало — левый верхний угол самой коробки. Это не
    // домысел: `offset-distance-001` даёт `path('m 0 0 h 200 v 150 z')` и
    // `offset-distance: 20%`, а эталон — `translateX(120px)`, то есть отсчёт
    // от места ЭЛЕМЕНТА (коробка стоит на (8,8) от начального блока).
    if let Some(inner) = func.strip_prefix("path(") {
        let d = inner
            .trim_end_matches(')')
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        return path_css(c, &d, (0.0, 0.0));
    }
    if let Some(args) = func.strip_prefix("ray(") {
        return ray_css(c, args.trim_end_matches(')'), cb, cb.map(|g| g.boxes[kind]));
    }
    // `url(#id)` (motion-1 §offset-path): путь — эквивалентный путь SVG-фигуры,
    // а «The <coord-box> defines the viewport and user coordinate system for
    // the shape element, with the origin … at the top left corner, and units
    // being 1px in size» — то есть та же опорная коробка содержащего блока,
    // что у `<basic-shape>`. Не фигура (или нет такого `id`) — `path("m 0 0")`
    // в своей системе (`offset-path-url-011`). Отрезается всё до `#`: разбор
    // значения мог дописать к ссылке адрес документа.
    if let Some(inner) = func.strip_prefix("url(") {
        let raw = inner
            .trim_end_matches(')')
            .trim()
            .trim_matches(|q: char| q == '"' || q == '\'');
        let id = raw.rsplit_once('#').map_or(raw, |(_, t)| t);
        let Some(d) = SVG_SHAPES.with(|m| m.borrow().get(id).cloned()) else {
            return path_css(c, "m 0 0", (0.0, 0.0));
        };
        let g = cb?;
        let rb = g.boxes[kind];
        return path_css(c, &d, (rb.0 - g.self_off.0, rb.1 - g.self_off.1));
    }
    // Всё прочее — `<basic-shape>` или голый `<coord-box>`: и то и другое
    // живёт в опорной коробке содержащего блока, без неё строить нечего.
    let g = cb?;
    let rb = g.boxes[kind];
    let shift = (rb.0 - g.self_off.0, rb.1 - g.self_off.1);
    let shape = shape_start(&func, c, g, rb);
    if shape.starts_with("circle(") || shape.starts_with("ellipse(") {
        return ellipse_path::css(c, &shape, rb.2, rb.3, shift);
    }
    let d = if func.is_empty() {
        // Голый `<coord-box>` = `inset(0 round X)` (§offset-path); слово
        // передаём любое — `rrect_of` берёт из него только размер и радиусы.
        crate::paint::background::motion_shape_d("border-box", rb.2, rb.3, g.radius)?
    } else {
        crate::paint::background::motion_shape_d(&shape, rb.2, rb.3, g.radius)?
    };
    // Путь строится в системе ОПОРНОЙ коробки, а `transform` живёт в системе
    // самой коробки: сдвигаем на разницу их начал.
    path_css(c, &d, shift)
}

/// Разделить `<offset-path> || <coord-box>` (§offset-path «Value: none |
/// `<offset-path>` || `<coord-box>`»): слово-коробка может стоять и до, и
/// после функции. Возвращает саму функцию (может быть пустой — это голый
/// `<coord-box>`) и номер коробки; умолчание — `border-box`.
pub(super) fn split_coord_box(raw: &str) -> (String, usize) {
    // css-box-4 §coord-box: у коробок CSS `fill-box` ведёт себя как
    // content-box, `stroke-box` и `view-box` — как border-box.
    const BOXES: [(&str, usize); 7] = [
        ("margin-box", 0),
        ("border-box", 1),
        ("padding-box", 2),
        ("content-box", 3),
        ("fill-box", 3),
        ("stroke-box", 1),
        ("view-box", 1),
    ];
    let mut s = raw.trim().to_string();
    let mut kind = 1usize;
    for (word, k) in BOXES {
        let Some(at) = s.find(word) else { continue };
        // Слово ищем только ВНЕ скобок: внутри `inset(… round …)` его быть не
        // может, а `ray(…) content-box` — снаружи.
        if s[..at].matches('(').count() != s[..at].matches(')').count() {
            continue;
        }
        kind = k;
        s.replace_range(at..at + word.len(), "");
        break;
    }
    (s.trim().to_string(), kind)
}

/// Эквивалентный путь `<basic-shape>` в системе опорной коробки w×h
/// (§«Equivalent Paths For `<basic-shape>`»).
///
/// Круг и эллипс: «starts at the rightmost point … four circular arcs, each
/// comprising a quarter of the circle/ellipse, proceeding clockwise». Именно
/// поэтому здесь не годится `border_shape_path`: он пишет контур от ЛЕВОЙ
/// точки и против часовой — точка на 25 % оказалась бы в зеркальном месте.
/// Прямоугольники и многоугольник отдаёт `motion_shape_d`: `rrect_d` уже
/// начинает с левого конца верхней стороны и идёт по часовой.
/// Пропущенный `at <position>` у круга и эллипса: «if they accept an `at
/// <position>` argument but that argument is omitted, and the element defines
/// an offset starting position via 'offset-position', it uses the specified
/// offset starting position for that argument» (motion-1 §offset-path).
/// Начало даёт `start_of` в системе содержащего блока — в запись оно уходит
/// в системе опорной коробки (`offset-path-shape-circle-002`, `-ellipse-002`).
pub(super) fn shape_start(func: &str, c: &Computed, g: &Cb, rb: (f32, f32, f32, f32)) -> String {
    let pos = super::motion_style(c)
        .offset_position
        .as_deref()
        .map(str::trim)
        .unwrap_or("normal");
    let round = func.starts_with("circle(") || func.starts_with("ellipse(");
    let Some(open) = func.find('(') else {
        return func.to_string();
    };
    let body = func[open + 1..].trim_end_matches(')').trim();
    if !round || pos == "normal" || body.split_whitespace().any(|t| t == "at") {
        return func.to_string();
    }
    let s = start_of(c, Some(g), Some(rb));
    let (x, y) = (s.0 - rb.0, s.1 - rb.1);
    let head = &func[..open + 1];
    if body.is_empty() {
        format!("{head}at {x}px {y}px)")
    } else {
        format!("{head}{body} at {x}px {y}px)")
    }
}

/// Точка на разложенном пути и готовая строка `transform`. `shift` — перенос
/// из системы пути в систему коробки.
pub(super) fn path_css(c: &Computed, d: &str, shift: (f32, f32)) -> Option<String> {
    let poly = flatten(d)?;
    let total = length(&poly.pts);
    let want = match super::motion_style(c).offset_distance {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(p)) => p * total,
        // Смесь долей и точек: доля — от длины пути (css-values-4 §10.9).
        Some(Len::Calc(i)) => crate::style::values::value::calc_get(i)
            .pct_px()
            .map_or(0.0, |(p, px)| p * total + px),
        _ => 0.0,
    };
    // §path-distance: замкнутый контур — по модулю длины («Modulo here uses
    // the traditional mathematical definition, where the output is always
    // non-negative»), открытый — зажимается нулём и длиной.
    let s = if poly.closed && total > 0.0 {
        want.rem_euclid(total)
    } else {
        want.clamp(0.0, total)
    };
    let (p, ang) = sample(&poly.pts, s);
    Some(origin_shift(
        c,
        (p.0 + shift.0, p.1 + shift.1),
        rotation(c, ang),
    ))
}
