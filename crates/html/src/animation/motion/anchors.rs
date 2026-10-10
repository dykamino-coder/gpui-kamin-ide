//! Позиции и точки привязки преобразования motion path.

use super::{Cb, angle_rad};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// `offset-rotate: [ auto | reverse ] || <angle>` (§offset-rotate).
///
/// `auto` (начальное) — «the difference between the offset path's direction
/// at the offset position and the direction of the positive X axis»;
/// `reverse` — то же плюс пол-оборота; записанный угол ПРИБАВЛЯЕТСЯ к тому и
/// к другому («If specified with an `<angle>`, the angle is added to the
/// rotation component»), а сам по себе — просто поворот. Прежняя запись
/// прибавку к `reverse` не читала вовсе.
pub(super) fn rotation(c: &Computed, tangent: f32) -> f32 {
    let v = super::motion_style(c)
        .offset_rotate
        .as_deref()
        .map(str::trim)
        .unwrap_or("auto");
    let (base, rest) = if let Some(r) = v.strip_prefix("auto") {
        (tangent, r)
    } else if let Some(r) = v.strip_prefix("reverse") {
        (tangent + std::f32::consts::PI, r)
    } else {
        (0.0, v)
    };
    base + rest
        .split_whitespace()
        .next()
        .and_then(angle_rad)
        .unwrap_or(0.0)
}

/// `<position>` в коробке `rb` (css-values-4 §position): слова, длины, доли.
/// Одно значение задаёт X, второе — Y; горизонтальное слово всегда идёт в X,
/// как бы ни стояло в записи (`at top left` — это `left top`).
pub(super) fn position_in(toks: &[&str], rb: (f32, f32, f32, f32)) -> (f32, f32) {
    let axis = |t: &str, base: f32| -> f32 {
        match t {
            "left" | "top" => 0.0,
            "right" | "bottom" => base,
            "center" => base / 2.0,
            _ => match Len::parse(t) {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) => k * base,
                _ => base / 2.0,
            },
        }
    };
    let horiz = |t: &str| matches!(t, "left" | "right");
    let vert = |t: &str| matches!(t, "top" | "bottom");
    let (a, b) = match (toks.first(), toks.get(1)) {
        (Some(a), Some(b)) if vert(a) || horiz(b) => (*b, *a),
        (Some(a), Some(b)) => (*a, *b),
        // Одно вертикальное слово — ось Y, X по центру (css-values-4 §position).
        (Some(a), None) if vert(a) => ("center", *a),
        (Some(a), None) => (*a, "center"),
        _ => ("center", "center"),
    };
    (rb.0 + axis(a, rb.2), rb.1 + axis(b, rb.3))
}

/// Начало пути, когда функция своего не задала: `offset-position`
/// (§offset-position). `auto` — «the top-left corner of the box», то есть
/// начало СВОЕЙ системы координат; `<position>` — точка в содержащем блоке;
/// `normal` (начальное) — начала нет, и `ray()` ведёт себя как `at center`
/// (§ray()/at). Без геометрии содержащего блока остаётся только `auto`.
pub(super) fn start_of(
    c: &Computed,
    cb: Option<&Cb>,
    rb: Option<(f32, f32, f32, f32)>,
) -> (f32, f32) {
    let v = super::motion_style(c)
        .offset_position
        .as_deref()
        .map(str::trim)
        .unwrap_or("normal");
    let own = cb.map_or((0.0, 0.0), |g| g.self_off);
    match (v, rb) {
        ("auto", _) => own,
        ("normal", Some(rb)) => (rb.0 + rb.2 / 2.0, rb.1 + rb.3 / 2.0),
        (_, Some(rb)) => position_in(&v.split_whitespace().collect::<Vec<_>>(), rb),
        // Опорной коробки нет: точку в ней не выразить, остаётся своё начало.
        _ => own,
    }
}

/// Сборка строки `translate(P) translate(−O) rotate(A) translate(O − Anchor)`.
///
/// Поворот идёт вокруг точки отсчёта преобразования, поэтому вычитание `O`
/// пишется ОТДЕЛЬНЫМИ звеньями: доля своего размера выражается процентным
/// `translate()`, а `calc()` разборщик `transform` не знает.
/// `offset-anchor: auto` (начальное) совпадает с точкой отсчёта (§offset-anchor:
/// «The anchor point is the same as the transform-origin») — хвостовой сдвиг
/// `O − Anchor` тогда нулевой и не пишется вовсе, и строка для сегодняшних
/// зелёных не меняется ни байтом.
pub(super) fn origin_shift(c: &Computed, p: (f32, f32), rot: f32) -> String {
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
    // Хвост пишется В СИСТЕМЕ ПОВЁРНУТОЙ коробки — так же, как это делают
    // эталоны набора (`offset-path-shape-rect-001-ref`:
    // `translate(520px,142px) rotate(90deg) translate(40px,40px)`).
    if let Some(((axp, axc), (ayp, ayc))) = anchor_point(c) {
        let oxp = opx.unwrap_or(0.0);
        let oyp = opy.unwrap_or(0.0);
        let oxc = if opx.is_none() { ofx * 100.0 } else { 0.0 };
        let oyc = if opy.is_none() { ofy * 100.0 } else { 0.0 };
        if oxp != axp || oyp != ayp {
            css.push_str(&format!(" translate({}px, {}px)", oxp - axp, oyp - ayp));
        }
        if oxc != axc {
            css.push_str(&format!(" translateX({}%)", oxc - axc));
        }
        if oyc != ayc {
            css.push_str(&format!(" translateY({}%)", oyc - ayc));
        }
    }
    css
}

/// Точка привязки (`offset-anchor`) как пара «точки, проценты» по каждой оси.
/// `None` — `auto` (начальное) или не задано: привязка равна `transform-origin`.
///
/// Проценты НЕ сворачиваются в пиксели намеренно: §offset-anchor меряет их «to
/// the width and the height of the element's reference box», а собственный
/// размер коробки известен только на отрисовке — там же, где `transform`
/// решает `translateX(%)`.
pub(super) fn anchor_point(c: &Computed) -> Option<((f32, f32), (f32, f32))> {
    let v = super::motion_style(c)
        .offset_anchor
        .as_deref()
        .map(str::trim)?;
    if v.is_empty() || v == "auto" {
        return None;
    }
    let toks: Vec<&str> = v.split_whitespace().collect();
    let axis = |t: &str| -> (f32, f32) {
        match t {
            "left" | "top" => (0.0, 0.0),
            "right" | "bottom" => (0.0, 100.0),
            "center" => (0.0, 50.0),
            _ => match Len::parse(t) {
                Some(Len::Px(x)) => (x, 0.0),
                Some(Len::Pct(k)) => (0.0, k * 100.0),
                _ => (0.0, 50.0),
            },
        }
    };
    let horiz = |t: &str| matches!(t, "left" | "right");
    let vert = |t: &str| matches!(t, "top" | "bottom");
    let (a, b) = match (toks.first(), toks.get(1)) {
        (Some(a), Some(b)) if vert(a) || horiz(b) => (*b, *a),
        (Some(a), Some(b)) => (*a, *b),
        // `offset-anchor: top` — это `center top` (css-values-4 §position).
        (Some(a), None) if vert(a) => ("center", *a),
        (Some(a), None) => (*a, "center"),
        _ => ("center", "center"),
    };
    Some((axis(a), axis(b)))
}
