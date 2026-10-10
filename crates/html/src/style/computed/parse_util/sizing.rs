//! Разбор размеров: fit-content(), calc-size(), view-box, назначение размера, flex-коэффициенты.

use super::*;

/// Довод `fit-content(<length-percentage>)` (css-sizing-3 §4.1): только
/// точки или доля, неотрицательные. Прочее (`em`, `calc`) — `None`, и
/// значение ведёт себя, как прежде, голым `fit-content`.
pub(in crate::style::computed) fn fit_content_arg(v: &str) -> Option<Len> {
    let lower = v.trim().to_ascii_lowercase();
    let arg = lower.strip_prefix("fit-content(")?.strip_suffix(')')?;
    match Len::parse(arg) {
        Some(l @ (Len::Px(n) | Len::Pct(n))) if n >= 0.0 => Some(l),
        _ => None,
    }
}

/// Разобранный `calc-size()`.
pub(in crate::style::computed) enum CalcSize {
    /// Основа — длина: значение известно сразу.
    Fixed(f32),
    /// Основа — ключевое слово размера: `(mul, add, max, min)` над ним.
    Over((f32, f32, f32, f32)),
}

/// `calc-size(<basis>, <calc-sum>)` (css-values-5 §calc-size,
/// `csswg-drafts/css-values-5/Overview.bs`). Понимаются линейные выражения
/// над `size` (`size`, `size ± L`, `size * k`, `k * size`, `size / k`, их
/// суммы) и `min(size, L)` / `max(size, L)`; длины — в точках. Основа —
/// `auto`, `fit-content`, `min-content`, `max-content`, `content` или длина;
/// вложенный `calc-size()` и проценты не понимаются — объявление роняется.
pub(in crate::style::computed) fn calc_size_arg(v: &str) -> Option<CalcSize> {
    let inner = v.trim().strip_prefix("calc-size(")?.strip_suffix(')')?;
    let (basis, expr) = inner.split_once(',')?;
    let basis = basis.trim();
    let mut expr: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(e) = expr.strip_prefix("calc(").and_then(|e| e.strip_suffix(')')) {
        expr = e.to_string();
    }
    let px = |t: &str| t.strip_suffix("px").and_then(|n| n.parse::<f32>().ok());
    let f = if let Some(a) = expr
        .strip_prefix("min(size,")
        .and_then(|e| e.strip_suffix(')'))
    {
        (1.0, 0.0, px(a)?, f32::MIN)
    } else if let Some(a) = expr
        .strip_prefix("max(size,")
        .and_then(|e| e.strip_suffix(')'))
    {
        (1.0, 0.0, f32::MAX, px(a)?)
    } else {
        // Сумма членов: `size`, `size*k`, `k*size`, `size/k`, `L`.
        let (mut mul, mut add) = (0.0f32, 0.0f32);
        let mut rest = expr.as_str();
        let mut sign = 1.0f32;
        if let Some(r) = rest.strip_prefix('-') {
            sign = -1.0;
            rest = r;
        }
        loop {
            let end = rest.find(['+', '-']).unwrap_or(rest.len());
            let term = &rest[..end];
            if term == "size" {
                mul += sign;
            } else if let Some(k) = term
                .strip_prefix("size*")
                .or_else(|| term.strip_suffix("*size"))
            {
                mul += sign * k.parse::<f32>().ok()?;
            } else if let Some(k) = term.strip_prefix("size/") {
                mul += sign / k.parse::<f32>().ok()?;
            } else {
                add += sign * px(term)?;
            }
            if end == rest.len() {
                break;
            }
            sign = if rest.as_bytes()[end] == b'-' {
                -1.0
            } else {
                1.0
            };
            rest = &rest[end + 1..];
        }
        (mul, add, f32::MAX, f32::MIN)
    };
    if let Some(b) = px(basis) {
        let (mul, add, max, min) = f;
        return Some(CalcSize::Fixed((b * mul + add).min(max).max(min).max(0.0)));
    }
    matches!(
        basis,
        "auto" | "fit-content" | "min-content" | "max-content" | "content"
    )
    .then_some(CalcSize::Over(f))
}

/// `xywh()` (css-images-4 §object-view-box; css-shapes-1 §basic-shape-rect).
/// Длины — точки или доли; `inset` с 1-3 значениями раскрывается как поля.
pub(in crate::style::computed) fn parse_view_box(v: &str) -> Option<(u8, [Len; 4])> {
    let v = v.trim();
    let (kind, inner) = if let Some(r) = v.strip_prefix("inset(") {
        (0u8, r)
    } else if let Some(r) = v.strip_prefix("rect(") {
        (1u8, r)
    } else if let Some(r) = v.strip_prefix("xywh(") {
        (2u8, r)
    } else {
        return None;
    };
    let inner = inner.strip_suffix(')')?;
    let parts: Vec<Len> = inner
        .split_whitespace()
        .map(|t| match Len::parse(t) {
            Some(l @ (Len::Px(_) | Len::Pct(_))) => Some(l),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let four = match (kind, parts.as_slice()) {
        (_, [a, b, c, d]) => [*a, *b, *c, *d],
        (0, [a]) => [*a, *a, *a, *a],
        (0, [a, b]) => [*a, *b, *a, *b],
        (0, [a, b, c]) => [*a, *b, *c, *b],
        _ => return None,
    };
    Some((kind, four))
}

pub(in crate::style::computed) fn assign_size(slot: &mut Option<Len>, v: &str) {
    // Смесь «доля ± точки» доживает индексом (`parse_mixed`): раскладка
    // складывает её сама (`DefiniteLength::Calc`, css-values-4 §10.9).
    // Прежде `calc(50% - 3px)` роняло объявление (`calc-width-block-1`).
    let parsed = size_range::parse(v);
    // `none` снимает предел (§10.4) — слот гаснет по праву. Прочая
    // неразборная запись объявление роняет: слот сохраняет прежнее значение,
    // а не гаснет (§4.2).
    if v.trim().eq_ignore_ascii_case("none") {
        *slot = None;
        return;
    }
    if let Some(l) = parsed {
        *slot = Some(l);
    }
}

/// Множитель роста/сжатия гибкого элемента: `<number>` или `calc()` из чисел
/// (css-values-4 §10.1), неотрицательный. `calc(infinity)` (§10.7.1) —
/// наибольшее представимое: здесь — конечное большое, чтобы сумма
/// множителей и доли свободного места не уходили в бесконечность и `NaN`
/// (`flex-grow-009`: `flex: calc(infinity) 0 0px` забирает всё место).
pub(in crate::style::computed) fn flex_factor(v: &str) -> Option<f32> {
    let g = crate::style::values::value::number(v)?;
    if g.is_nan() || g < 0.0 {
        return None;
    }
    Some(g.min(1.0e18))
}
