//! Разбор рамки и формы: overflow, border-shape, corner-shape, четыре стороны, ширина outline, скругление.

use super::*;

pub(in crate::style::computed) fn parse_overflow(v: &str) -> Option<Overflow> {
    match v {
        "hidden" => Some(Overflow::Hidden),
        "clip" => Some(Overflow::Clip),
        // `overlay` — устаревший синоним `auto` (css-overflow-3 §overflow:
        // «legacy value alias of auto»; `overflow-overlay`).
        "scroll" | "auto" | "overlay" => Some(Overflow::Scroll),
        "visible" => Some(Overflow::Visible),
        _ => None,
    }
}

/// Опорная коробка `<geometry-box>` (css-masking-1 §1.3.1.1 плюс
/// `half-border-box` css-borders-4): 0 border, 1 margin, 2 padding,
/// 3 content, 4 half-border. У элемента с CSS-коробкой `fill-box` =
/// content-box, `stroke-box`/`view-box` = border-box.
fn geometry_box_kind(word: &str) -> Option<u8> {
    Some(match word.trim().to_ascii_lowercase().as_str() {
        "border-box" | "stroke-box" | "view-box" => 0,
        "margin-box" => 1,
        "padding-box" => 2,
        "content-box" | "fill-box" => 3,
        "half-border-box" => 4,
        _ => return None,
    })
}

/// `border-shape: [ <basic-shape> <geometry-box>? ]{1,2}`. Фигура — функция
/// со скобками, режется по ПАРНОЙ закрывающей (внутри `polygon(...)`
/// запятые, внутри `path('...')` — что угодно); слово коробки — следом за
/// ней. Хвост, не разобранный в две фигуры, делает значение недействительным.
pub(in crate::style::computed) fn parse_border_shape(v: &str) -> Option<BorderShape> {
    let mut items: Vec<(String, Option<u8>)> = Vec::new();
    let mut rest = v.trim();
    while !rest.is_empty() && items.len() < 2 {
        let open = rest.find('(')?;
        let mut depth = 0usize;
        let mut close = None;
        for (i, ch) in rest[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close?;
        let shape = rest[..=close].trim().to_string();
        // Прямоугольные фигуры пишутся через пробел (css-shapes-1 §basic-shape:
        // `rect( [ <length-percentage> | auto ]{4} … )`, так же `inset()` и
        // `xywh()`); запятая делает всё объявление недействительным, и
        // `border-shape` остаётся `none` — Blink `ConsumeBasicShapeRect`
        // (`css_parsing_utils.cc:651-668`) берёт четыре длины подряд без
        // запятой. Прежде `rect(0, 0, 100%, 100%)` разбирался в пустой
        // прямоугольник, и маска фигуры прятала коробку целиком
        // (border-shape-inset-shadow-blur, -negative-spread: пустая страница).
        let head = shape[..open].trim_start().to_ascii_lowercase();
        if matches!(head.as_str(), "rect" | "inset" | "xywh") && shape.contains(',') {
            return None;
        }
        rest = rest[close + 1..].trim_start();
        let word_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let bx = geometry_box_kind(&rest[..word_end]);
        if bx.is_some() {
            rest = rest[word_end..].trim_start();
        }
        items.push((shape, bx));
    }
    if !rest.is_empty() {
        return None;
    }
    let mut it = items.into_iter();
    let (outer, outer_box) = it.next()?;
    Some(match it.next() {
        Some((inner, inner_box)) => BorderShape {
            outer,
            outer_box: outer_box.unwrap_or(0),
            inner: Some((inner, inner_box.unwrap_or(2))),
        },
        None => BorderShape {
            outer,
            outer_box: outer_box.unwrap_or(4),
            inner: None,
        },
    })
}

/// Параметр суперэллипса из одного значения `<corner-shape-value>`
/// (css-borders-4 §corner-shaping): ключевые слова — их числовые
/// эквиваленты по спеке, `superellipse(<number> | infinity | -infinity)` —
/// само число. `None` — не форма угла (запись отбрасывается).
pub(in crate::style::computed) fn corner_shape_param(tok: &str) -> Option<f32> {
    let t = tok.trim().to_ascii_lowercase();
    Some(match t.as_str() {
        "round" => 1.0,
        "squircle" => 2.0,
        "square" => f32::INFINITY,
        "bevel" => 0.0,
        "scoop" => -1.0,
        "notch" => f32::NEG_INFINITY,
        _ => {
            let inner = t.strip_prefix("superellipse(")?.strip_suffix(')')?.trim();
            match inner {
                "infinity" => f32::INFINITY,
                "-infinity" => f32::NEG_INFINITY,
                n => n.parse::<f32>().ok()?,
            }
        }
    })
}

/// `corner-shape: a [b [c [d]]]` → K по углам tl/tr/br/bl — раскладка та же,
/// что у `border-radius` (§corner-shaping-shorthand). Функции со скобками
/// внутри пробелов не содержат, поэтому режем по пробелам.
pub(in crate::style::computed) fn corner_shape_shorthand(raw: &str) -> Option<[f32; 4]> {
    let v: Vec<f32> = raw
        .split_whitespace()
        .map(corner_shape_param)
        .collect::<Option<Vec<_>>>()?;
    Some(match v.len() {
        1 => [v[0]; 4],
        2 => [v[0], v[1], v[0], v[1]],
        3 => [v[0], v[1], v[2], v[1]],
        4 => [v[0], v[1], v[2], v[3]],
        _ => return None,
    })
}

/// Смещение первой запятой ВНЕ вложенных скобок.
/// Раскрытие записи в четыре стороны: 1 значение — все, 2 — верт/гориз,
/// 3 — верх/гориз/низ, 4 — по часовой. `None`, если разобрать не удалось.
pub(in crate::style::computed) fn four<T: Copy>(
    words: &[&str],
    one: impl Fn(&str) -> Option<T>,
) -> Option<[T; 4]> {
    let v: Vec<T> = words.iter().filter_map(|w| one(w)).collect();
    match v.len() {
        1 => Some([v[0]; 4]),
        2 => Some([v[0], v[1], v[0], v[1]]),
        3 => Some([v[0], v[1], v[2], v[1]]),
        4 => Some([v[0], v[1], v[2], v[3]]),
        _ => None,
    }
}

/// Ширина обводки: ключевые слова и любые шрифтовые/абсолютные длины.
pub(in crate::style::computed) fn outline_width_of(v: &str) -> Option<Len> {
    match v {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => Len::parse(v).filter(|l| !matches!(l, Len::Pct(_))),
    }
}

/// `object-view-box: none | <basic-shape-rect>` — `inset()`, `rect()`,
/// `round <'border-radius'>` of `inset()`/`rect()`/`xywh()` (css-shapes-1
/// §basic-shape-rect): one radius for all corners and both axes — every
/// listed value, before and after `/`, equal. `20px / 20px` is that radius;
/// unequal corners are not representable here and stay unrounded.
pub(in crate::style::computed) fn uniform_round(r: &str) -> Option<Len> {
    let mut it = r
        .split(|c: char| c == '/' || c.is_whitespace())
        .filter(|t| !t.is_empty());
    let first = Len::parse(it.next()?)?;
    it.all(|t| Len::parse(t) == Some(first)).then_some(first)
}
