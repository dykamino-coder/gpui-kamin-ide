//! `shape()` (css-shapes-2 §2.4) в команды контура SVG относительно опорной коробки.

use crate::document::state::ValueCx;
use crate::style::computed::FontCx;
use crate::style::css::value_tokens::split_top;
use crate::svg::path_commands::{Command, Sink};

/// Перевести `shape()` (css-shapes-2 §2.4) в контур SVG `d`.
///
/// Команды идут через точку с запятой (запятые заменил разбор свойств —
/// по запятым верхнего уровня режутся слои маски). Доли резолвятся здесь:
/// x — от ширины коробки, y — от высоты.
pub fn shape_to_path(args: &str, bw: f32, bh: f32, vcx: ValueCx<'_>) -> Option<String> {
    let mut d = String::new();
    shape_commands(args, bw, bh, &mut d, vcx)?;
    (!d.is_empty()).then(|| d.trim_end().to_string())
}

pub(crate) fn shape_commands(
    args: &str,
    bw: f32,
    bh: f32,
    d: &mut impl Sink,
    vcx: ValueCx<'_>,
) -> Option<()> {
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
        if let Some((p, add)) = crate::style::values::value::calc_pct_px(t, vcx) {
            return Some(p * side + add);
        }
        match crate::style::values::value::Len::parse(t, vcx)? {
            crate::style::values::value::Len::Px(v) => Some(v),
            crate::style::values::value::Len::Pct(p) => Some(p * side),
            // Шрифтовые единицы — от запасного кегля (`FontCx::FALLBACK`).
            l => l.to_px(None, FontCx::FALLBACK, vcx),
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
                d.command(rel, Command::Move([x, y]));
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                sub = cur;
            }
            "line" => {
                let rel = toks.get(1) == Some(&"by");
                let (x, y) = pair(toks.get(2)?, toks.get(3)?)?;
                d.command(rel, Command::Line([x, y]));
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
            }
            "hline" => {
                let rel = toks.get(1) == Some(&"by");
                let x = val(toks.get(2)?, bw)?;
                d.command(rel, Command::Horizontal(x));
                cur.0 = if rel { cur.0 + x } else { x };
            }
            "vline" => {
                let rel = toks.get(1) == Some(&"by");
                let y = val(toks.get(2)?, bh)?;
                d.command(rel, Command::Vertical(y));
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
                    d.command(
                        rel,
                        Command::Cubic([
                            c1.0 - base.0,
                            c1.1 - base.1,
                            c2.0 - base.0,
                            c2.1 - base.1,
                            x,
                            y,
                        ]),
                    );
                } else {
                    d.command(
                        rel,
                        Command::Quadratic([c1.0 - base.0, c1.1 - base.1, x, y]),
                    );
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
                    d.command(
                        rel,
                        Command::SmoothCubic([c.0 - base.0, c.1 - base.1, x, y]),
                    );
                } else {
                    d.command(rel, Command::SmoothQuadratic([x, y]));
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
                d.command(
                    rel,
                    Command::Arc([rx, ry, rot, large as f32, sweep as f32, x, y]),
                );
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
            }
            // `close` возвращает перо в начало подконтура — следующий
            // сегмент считает свой якорь `start` уже оттуда.
            "close" => {
                d.command(false, Command::Close);
                cur = sub;
            }
            _ => return None,
        }
    }
    Some(())
}
