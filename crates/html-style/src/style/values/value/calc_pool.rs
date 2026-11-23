//! Immutable calc sums, interpolation, simple folding, and numeric expressions.

use super::*;
use crate::values::ValueCx;

/// Immutable sum shared by exactly the styles and frames that read it.
pub type CalcId = crate::values::ValueRef<Sum>;

pub fn calc_store(s: Sum, _vcx: ValueCx<'_>) -> CalcId {
    CalcId::new(s)
}

/// Margins and padding resolve percentages against the containing block's
/// inline size, which layout always knows (CSS 2.1 §§8.3, 8.4), so a
/// percentage term cancelled to zero (`calc(0% + 30px)`) contributes nothing
/// and the length alone remains. The kept percentage type only matters where
/// an indefinite basis changes behaviour (heights, insets: CSS Values 4 §10.11).
pub fn fold_zero_percentage(l: Option<Len>, vcx: ValueCx<'_>) -> Option<Len> {
    match l {
        Some(Len::Calc(ref i)) => {
            let s = calc_get(i, vcx);
            if s.has_percentage && s.pct == 0.0 {
                Sum {
                    has_percentage: false,
                    ..s
                }
                .collapse(vcx)
                .or(l)
            } else {
                l
            }
        }
        _ => l,
    }
}

/// Reading an expression does not depend on the document providing its bases.
pub fn calc_get(i: impl std::borrow::Borrow<CalcId>, _vcx: ValueCx<'_>) -> Sum {
    *i.borrow().read()
}

/// Интерполяция длин РАЗНЫХ природ покомпонентно, как `calc()` (css-values-4
/// §3.2): середина `0px → 200vw` — `100vw`. Результат сворачивается обычным
/// `collapse`; смесь, которую он не держит (доля вместе с точками), даёт
/// `None` — вызывающий берёт ближайший кадр, как прежде.
pub fn lerp_len(a: Len, b: Len, k: f32, vcx: ValueCx<'_>) -> Option<Len> {
    let (a, b) = (Sum::from_len(a, vcx)?, Sum::from_len(b, vcx)?);
    a.scaled(1.0 - k).add(b.scaled(k), 1.0).collapse(vcx)
}

/// Число `<number>` или `calc()` из одних чисел (css-values-4 §10.1).
///
/// `font-size-adjust: cap-height calc(1462 / 2048)` — не длина, и
/// `Len::parse` её не берёт: голое число `eval_calc` отбрасывает намеренно.
pub fn number(s: &str, vcx: ValueCx<'_>) -> Option<f32> {
    let s = s.trim();
    let body = s
        .strip_prefix("calc(")
        .and_then(|r| r.strip_suffix(')'))
        // `pow(2, 3)`, `round(2.5)` — числовые функции верхнего уровня.
        .or_else(|| super::calc_math::is_math_fn(s).then_some(s));
    if let Some(inner) = body {
        let mut calc = Calc::new(inner);
        return match calc.expr(vcx)? {
            // Верхний уровень: NaN -> 0 (css-values-4 §10.13).
            Val::Num(n) if calc.rest.trim().is_empty() => Some(if n.is_nan() { 0.0 } else { n }),
            _ => None,
        };
    }
    css_number(s)
}

/// Read a percentage/pixel calc pair without allocating an expression. Paint consumers call this every frame; other units return None.
pub fn calc_pct_px(raw: &str, vcx: ValueCx<'_>) -> Option<(f32, f32)> {
    let inner = raw.trim().strip_prefix("calc(")?.strip_suffix(')')?;
    let s = Calc::new(inner).sum(vcx)?;
    s.length_percentage()
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
