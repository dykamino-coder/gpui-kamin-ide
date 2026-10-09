//! Контуры фигур: shape_to_path, border-shape (путь, маска, кольцо, тень, обводка), rrect.

use crate::paint::background::*;
use crate::style::computed::Computed;

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
        if let Some((p, add)) = crate::style::values::value::calc_pct_px(t) {
            return Some(p * side + add);
        }
        match crate::style::values::value::Len::parse(t)? {
            crate::style::values::value::Len::Px(v) => Some(v),
            crate::style::values::value::Len::Pct(p) => Some(p * side),
            // Шрифтовые единицы — от запасного кегля (16px).
            l => crate::text::metrics::fallback_len_px(l, "", 16.0),
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
pub(crate) fn miter_limit(raw: &str) -> f32 {
    if shape_is_linear(raw) { 1000.0 } else { 4.0 }
}

/// Скруглённый прямоугольник → контур `d` дугами; радиусы жмутся одним
/// множителем (css-backgrounds-3 §5.5), как в `rrect_mask`. Вырожденный
/// прямоугольник — пустой контур.
pub(crate) fn rrect_d((x0, y0, w, h): (f32, f32, f32, f32), radii: [(f32, f32); 4]) -> String {
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
    colour: crate::style::values::value::Color,
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
    shadows: &[(crate::style::computed::Shadow, crate::style::values::value::Color)],
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
    let rgb = |c: crate::style::values::value::Color| {
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
    colour: crate::style::values::value::Color,
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
