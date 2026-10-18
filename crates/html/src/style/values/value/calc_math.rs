//! Математические функции calc() (css-values-4 §10.3–§10.8, §10.7.1): round/mod/rem, тригонометрия, pow/sqrt/hypot/log/exp, abs/sign, константы e/pi/infinity/NaN.
//!
//! Длины решаются по ПРИРОДЕ: у всех доводов одна живая единица (`Sum::nature`),
//! и функция считается над её коэффициентами. Это точно, а не приближение:
//! базы единиц (`em`, `vw`, доля…) положительны, поэтому `round(A, B)`,
//! `mod`, `hypot`, `abs`, `sign` от `a·u` и `b·u` равны тем же функциям от
//! `a` и `b`, умноженным на `u`. Смесь природ (`round(1em, 3px)`) решается
//! только после каскада — такая запись, как и `min()` над смесью,
//! отбрасывается.

use super::Sum;
use super::calc::{Calc, Val};
use std::f32::consts::{E, PI};

const FNS: [&str; 17] = [
    "round", "mod", "rem", "sin", "cos", "tan", "asin", "acos", "atan2", "atan", "pow", "sqrt",
    "hypot", "log", "exp", "abs", "sign",
];

/// Начинается ли запись с вызова математической функции.
pub(crate) fn is_math_fn(s: &str) -> bool {
    math_name(s).is_some()
}

fn math_name(s: &str) -> Option<&'static str> {
    let open = s.find('(')?;
    let name = &s[..open];
    FNS.iter().copied().find(|f| f.eq_ignore_ascii_case(name))
}

/// Природа операнда и его величина: число, угол (радианы) или длина одной
/// природы. Числа и углы получают свои номера за пределами полей `Sum`.
pub(super) fn key(v: &Val) -> Option<(u8, f32)> {
    match v {
        Val::Num(n) => Some((u8::MAX, *n)),
        Val::Ang(a) => Some((u8::MAX - 1, *a)),
        Val::Len(s) => s.nature(),
    }
}

/// Значение природы `nature` с величиной `v` — обратное к `key`.
pub(super) fn rebuild(nature: u8, v: f32) -> Val {
    match nature {
        u8::MAX => Val::Num(v),
        n if n == u8::MAX - 1 => Val::Ang(v),
        n => Val::Len(Sum::of_nature(n, v)),
    }
}

/// Общая природа доводов и их величины. Нулевая длина (`0px`, пустая сумма)
/// совместима с любой длиной: `round(0em, 1em)` — однородная запись.
fn same(args: &[Val]) -> Option<(u8, [f32; 2])> {
    let mut common = super::calc_arguments::Homogeneous::default();
    let mut values = [0.0; 2];
    for (value, slot) in args.iter().zip(values.iter_mut()) {
        *slot = common.push(*value)?;
    }
    let nature = common.nature()?;
    Some((nature, values))
}

/// `round(<strategy>, A, B)` (§10.3, «Argument Ranges»).
fn round(strategy: &str, a: f32, b: f32) -> f32 {
    if b == 0.0 || (a.is_infinite() && b.is_infinite()) {
        return f32::NAN;
    }
    if a.is_infinite() {
        return a;
    }
    if b.is_infinite() {
        let zero = if a.is_sign_negative() { -0.0 } else { 0.0 };
        return match strategy {
            "up" if a > 0.0 => f32::INFINITY,
            "down" if a < 0.0 => f32::NEG_INFINITY,
            _ => zero,
        };
    }
    let q = a / b.abs();
    let (lo, hi) = (q.floor(), q.ceil());
    let n = match strategy {
        "up" => hi,
        "down" => lo,
        "to-zero" => q.trunc(),
        // Ровно посередине — к +∞ («the upper value»).
        _ if q - lo < hi - q => lo,
        _ => hi,
    };
    n * b.abs()
}

fn modulo(rem: bool, a: f32, b: f32) -> f32 {
    if b == 0.0 || a.is_infinite() {
        return f32::NAN;
    }
    if b.is_infinite() {
        // §10.3: у `mod()` знак A против знака B даёт NaN.
        return if rem || a == 0.0 || a.is_sign_negative() == b.is_sign_negative() {
            a
        } else {
            f32::NAN
        };
    }
    if rem { a % b } else { a - b * (a / b).floor() }
}

fn apply(name: &str, strategy: &str, args: &[Val]) -> Option<Val> {
    let num = |i: usize| match args.get(i)? {
        Val::Num(n) => Some(*n),
        _ => None,
    };
    let angle = |i: usize| match args.get(i)? {
        Val::Num(n) | Val::Ang(n) => Some(*n),
        Val::Len(_) => None,
    };
    let n = args.len();
    Some(match (name, n) {
        ("sin", 1) => Val::Num(angle(0)?.sin()),
        ("cos", 1) => Val::Num(angle(0)?.cos()),
        ("tan", 1) => Val::Num(angle(0)?.tan()),
        ("asin", 1) => Val::Ang(num(0)?.asin()),
        ("acos", 1) => Val::Ang(num(0)?.acos()),
        ("atan", 1) => Val::Ang(num(0)?.atan()),
        ("atan2", 2) => {
            let (_, v) = same(args)?;
            Val::Ang(v[0].atan2(v[1]))
        }
        ("pow", 2) => Val::Num(num(0)?.powf(num(1)?)),
        ("sqrt", 1) => Val::Num(num(0)?.sqrt()),
        ("exp", 1) => Val::Num(num(0)?.exp()),
        ("log", 1) => Val::Num(num(0)?.ln()),
        ("log", 2) => Val::Num(num(0)?.ln() / num(1)?.ln()),
        ("abs", 1) => {
            let (k, v) = key(&args[0])?;
            rebuild(k, v.abs())
        }
        ("sign", 1) => {
            let (_, v) = key(&args[0])?;
            Val::Num(if v == 0.0 || v.is_nan() {
                v
            } else {
                v.signum()
            })
        }
        // Без B — только для числа: шаг 1 (§10.3).
        ("round", 1) => Val::Num(round(strategy, num(0)?, 1.0)),
        ("round", 2) => {
            let (k, v) = same(args)?;
            rebuild(k, round(strategy, v[0], v[1]))
        }
        ("mod" | "rem", 2) => {
            let (k, v) = same(args)?;
            rebuild(k, modulo(name == "rem", v[0], v[1]))
        }
        _ => return None,
    })
}

impl Calc<'_> {
    /// Вызов математической функции в начале остатка (`is_math_fn` уже
    /// истинно). Доводы — полные выражения через запятую.
    pub(super) fn math_fn(&mut self) -> Option<Val> {
        let name = math_name(self.rest)?;
        let mut sub = Calc {
            rest: &self.rest[name.len() + 1..],
        };
        let mut strategy = "nearest";
        if name == "round" {
            let head = sub.rest.trim_start();
            for st in ["nearest", "up", "down", "to-zero"] {
                let Some(after) = head
                    .get(..st.len())
                    .filter(|h| h.eq_ignore_ascii_case(st))
                    .map(|_| &head[st.len()..])
                else {
                    continue;
                };
                if let Some(after) = after.trim_start().strip_prefix(',') {
                    strategy = st;
                    sub.rest = after;
                    break;
                }
            }
        }
        let result = if name == "hypot" {
            let mut common = super::calc_arguments::Homogeneous::default();
            let mut squares = 0.0;
            sub.arguments(|value| {
                let key = common.push(value)?;
                squares += key * key;
                Some(())
            })?;
            common.rebuild(squares.sqrt())
        } else {
            let mut args = [Val::Num(0.0); 2];
            let mut count = 0;
            sub.arguments(|value| {
                *args.get_mut(count)? = value;
                count += 1;
                Some(())
            })?;
            apply(name, strategy, &args[..count])
        };
        self.rest = sub.rest;
        result
    }
}

/// Константа или угол в одиночном операнде (§10.7.1, §7.1).
pub(super) fn constant_or_angle(head: &str) -> Option<Val> {
    let c = [
        ("e", E),
        ("pi", PI),
        ("infinity", f32::INFINITY),
        ("+infinity", f32::INFINITY),
        ("-infinity", f32::NEG_INFINITY),
        ("nan", f32::NAN),
    ]
    .into_iter()
    .find(|(name, _)| head.eq_ignore_ascii_case(name))
    .map(|(_, value)| value);
    if let Some(c) = c {
        return Some(Val::Num(c));
    }
    for (unit, k) in [
        ("grad", PI / 200.0),
        ("deg", PI / 180.0),
        ("rad", 1.0),
        ("turn", 2.0 * PI),
    ] {
        if let Some(v) = head.len().checked_sub(unit.len()).and_then(|at| {
            head.get(at..)
                .filter(|suffix| suffix.eq_ignore_ascii_case(unit))?;
            super::css_number(&head[..at])
        }) {
            return Some(Val::Ang(v * k));
        }
    }
    None
}
