//! Пул выражений calc() (Len::Calc хранит индекс), свёртка простых сумм, интерполяция и числа.

use super::*;

/// Арена смешанных сумм `calc()`: `Len` несёт индекс, не тело. Арена
/// append-only и копеечная (смеси редки); чистится вместе с документом
/// (`calc_reset` на входе разбора) — старые индексы умирают с его деревом.
static CALC_POOL: std::sync::Mutex<Vec<Sum>> = std::sync::Mutex::new(Vec::new());

pub fn calc_store(s: Sum) -> u32 {
    let mut pool = CALC_POOL.lock().unwrap();
    pool.push(s);
    (pool.len() - 1) as u32
}

/// Margins and padding resolve percentages against the containing block's
/// inline size, which layout always knows (CSS 2.1 §§8.3, 8.4), so a
/// percentage term cancelled to zero (`calc(0% + 30px)`) contributes nothing
/// and the length alone remains. The kept percentage type only matters where
/// an indefinite basis changes behaviour (heights, insets: CSS Values 4 §10.11).
pub fn fold_zero_percentage(l: Option<Len>) -> Option<Len> {
    match l {
        Some(Len::Calc(i)) => {
            let s = calc_get(i);
            if s.has_percentage && s.pct == 0.0 {
                Sum {
                    has_percentage: false,
                    ..s
                }
                .collapse()
                .or(l)
            } else {
                l
            }
        }
        _ => l,
    }
}

pub fn calc_get(i: u32) -> Sum {
    CALC_POOL
        .lock()
        .unwrap()
        .get(i as usize)
        .copied()
        .unwrap_or_default()
}

pub fn calc_reset() {
    CALC_POOL.lock().unwrap().clear();
}

/// Интерполяция длин РАЗНЫХ природ покомпонентно, как `calc()` (css-values-4
/// §3.2): середина `0px → 200vw` — `100vw`. Результат сворачивается обычным
/// `collapse`; смесь, которую он не держит (доля вместе с точками), даёт
/// `None` — вызывающий берёт ближайший кадр, как прежде.
pub fn lerp_len(a: Len, b: Len, k: f32) -> Option<Len> {
    let (a, b) = (Sum::from_len(a)?, Sum::from_len(b)?);
    a.scaled(1.0 - k).add(b.scaled(k), 1.0).collapse()
}

/// Число `<number>` или `calc()` из одних чисел (css-values-4 §10.1).
///
/// `font-size-adjust: cap-height calc(1462 / 2048)` — не длина, и
/// `Len::parse` её не берёт: голое число `eval_calc` отбрасывает намеренно.
pub fn number(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
        let mut calc = Calc { rest: inner };
        return match calc.expr()? {
            Val::Num(n) if calc.rest.trim().is_empty() => Some(n),
            _ => None,
        };
    }
    css_number(s)
}

/// `calc()` из долей и точек — парой `(доля, точки)`, БЕЗ записи в арену.
///
/// Для потребителей на отрисовке (`shape()`, центр `circle()`/`ellipse()`):
/// они зовутся каждый кадр, а арена `CALC_POOL` чистится только вместе с
/// документом — `parse_mixed` копил бы по записи на кадр. Любая другая
/// природа (`em`, `vw`…) — `None`.
pub fn calc_pct_px(raw: &str) -> Option<(f32, f32)> {
    let inner = raw.trim().strip_prefix("calc(")?.strip_suffix(')')?;
    let s = eval_calc(inner)?;
    let rest = Sum {
        px: 0.0,
        pct: 0.0,
        has_percentage: false,
        ..s
    };
    (rest == Sum::default()).then_some((s.pct, s.px))
}

/// Свернуть простые `calc(a op b)` (одно действие, единица не больше чем у
/// одного операнда) в число с единицей: компоненту цвета больше и не нужно.
/// Сложнее запись остаётся как есть — разбор её отвергнет, как и прежде.
pub(super) fn fold_simple_calc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find("calc(") {
        out.push_str(&rest[..at]);
        let body = &rest[at + 5..];
        let Some(end) = body.find(')') else {
            out.push_str(&rest[at..]);
            return out;
        };
        let expr = &body[..end];
        let split = |t: &str| -> Option<(f32, String)> {
            let t = t.trim();
            let cut = t
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
                .unwrap_or(t.len());
            Some((t[..cut].parse::<f32>().ok()?, t[cut..].to_string()))
        };
        let folded = ['*', '/', '+', '-'].iter().find_map(|op| {
            let (a, b) = expr.split_once(&format!(" {op} "))?;
            let ((x, ux), (y, uy)) = (split(a)?, split(b)?);
            let unit = if ux.is_empty() {
                uy.clone()
            } else {
                ux.clone()
            };
            if !ux.is_empty() && !uy.is_empty() && matches!(op, '*' | '/') {
                return None;
            }
            let v = match op {
                '*' => x * y,
                '/' if y != 0.0 => x / y,
                '+' if ux == uy => x + y,
                '-' if ux == uy => x - y,
                _ => return None,
            };
            Some(format!("{v}{unit}"))
        });
        match folded {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[at..at + 5 + end + 1]),
        }
        rest = &body[end + 1..];
    }
    out.push_str(rest);
    out
}
