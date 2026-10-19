//! Медиазапросы: Media (носитель задаёт документ), сравнения диапазонов.

use crate::document::state::ValueCx;
use crate::style::css::*;

mod calc;
mod comparison;
pub(crate) use comparison::MqCmp;
mod custom;
pub(crate) use custom::custom_media_of;
mod features;
mod query;
use features::{mq_and, mq_base, mq_or};

/// Разбор содержимого `<style>` — список правил в порядке появления.
/// Условия окружения для `@media`.
///
/// Ширина и высота — в точках, тема — тёмная или светлая. Без них правило
/// пропускалось целиком, и разметка, написанная от узкого экрана вверх,
/// навсегда оставалась в узком виде.
#[derive(Clone, Copy, Debug)]
pub struct Media {
    pub width: f32,
    pub height: f32,
    pub dark: bool,
    /// Печатный носитель: `@media print` истинен, `screen` — ложен.
    pub print: bool,
}

impl Default for Media {
    fn default() -> Self {
        Media::for_medium(false)
    }
}

impl Media {
    /// Экран или печать (`Document::new_media`; печать — `*-print`-тесты).
    pub fn for_medium(print: bool) -> Self {
        // Печать меряется ЛИСТОМ (mediaqueries-4 §width: «the width of the page
        // box»): лист WPT 5in x 3in = 480x288, `@page { size }` запрос не меняет
        // (csswg#5437; `media-queries-001-print` при `size: 10in`).
        let (width, height) = if print {
            (480.0, 288.0)
        } else {
            (1280.0, 800.0)
        };
        Media {
            width,
            height,
            dark: true,
            print,
        }
    }
}

/// Первая уравновешенная скобочная группа: тело БЕЗ внешних скобок и хвост.
/// Вложенные скобки (`(not (color))`, `calc(...)`) считаются, а не режутся
/// жадным `strip_suffix(')')`.
fn take_parens(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[1..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Число медиа-фичи: длина, отношение `a / b`, разрешение и голое число.
///
/// Возвращается ПАРА (числитель, знаменатель), а не частное: отношения
/// сравниваются перекрёстным умножением (css-values-4 §7.2 «If two ratios
/// need to be compared, divide the first number by the second» — но при
/// вырожденном `0/0` деление даёт NaN, и Blink считает крест, см.
/// `media_query_evaluator.cc::CompareAspectRatioValue`). Так `0/0`
/// сравнивается как `0 >= 0`, то есть истинно (`aspect-ratio-003`).
fn mq_operand(raw: &str, vcx: ValueCx<'_>) -> Option<(f32, f32)> {
    let s = raw.trim();
    // `calc(a / b)` — отношение внутри calc (`mq-calc-008`).
    let body = s
        .strip_prefix("calc(")
        .and_then(|r| r.strip_suffix(')'))
        .filter(|b| balanced(b))
        .unwrap_or(s);
    if let Some((a, b)) = split_top_slash(body)
        && let (Some(a), Some(b)) = (mq_scalar(a, vcx), mq_scalar(b, vcx))
    {
        // Вырожденное отношение с НУЛЕВЫМ знаменателем — бесконечность.
        // `0/0` спека обращает в `1/0` (css-values-4 §7.2 «degenerate
        // ratios»; ассерт `aspect-ratio-002` дословно: «0/0 (which is
        // converted into 1/0) is infinite»), иначе крест дал бы `0 >= 0`
        // и правило под `min-aspect-ratio: 0/0` применилось бы.
        if b == 0.0 {
            return Some((1.0, 0.0));
        }
        return Some((a, b));
    }
    mq_scalar(s, vcx).map(|v| (v, 1.0))
}

/// Скаляр: длина в точках, разрешение в dppx или голое число.
fn mq_scalar(raw: &str, vcx: ValueCx<'_>) -> Option<f32> {
    let s = raw.trim();
    // `calc()`/`sign()`/… — значение признака может быть любым выражением
    // (mediaqueries-4 §1.3, `mq-calc-sign-function-*`, `mq-calc-resolution`).
    if let Some((v, _)) = calc::eval(s) {
        return Some(v);
    }
    // Разрешение: `dppx` и его короткая запись `x`. Проверять надо ИМЕННО
    // через разбор остатка: `0px` тоже кончается на `x`, и жадное отрезание
    // давало «0p», разбор которого проваливался, а ранний возврат гасил всю
    // фичу (`at-media-whitespace-optional-002`, `mq-invalid-media-type-006`).
    if let Some(v) = s
        .strip_suffix("dppx")
        .or_else(|| s.strip_suffix('x'))
        .and_then(|n| n.trim().parse::<f32>().ok())
    {
        return Some(v);
    }
    // Разрешение внутри выражения (`calc(-1dppx * sign(…))`): `dppx` —
    // каноническая единица <resolution>, арифметика над ней та же, что над
    // числом (css-values-4 §7.4).
    if s.contains('(') && s.contains("dppx") {
        return crate::style::values::value::number(&s.replace("dppx", ""), vcx);
    }
    if let Some(n) = s.strip_suffix("dpi") {
        return n.trim().parse::<f32>().ok().map(|v| v / 96.0);
    }
    if let Some(n) = s.strip_suffix("dpcm") {
        return n.trim().parse::<f32>().ok().map(|v| v * 2.54 / 96.0);
    }
    match crate::style::values::value::Len::parse(s, vcx) {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        Some(crate::style::values::value::Len::Em(k)) => Some(k * 16.0),
        // Единицы шрифта в медиа-запросе берутся от НАЧАЛЬНОГО шрифта, а не
        // от корневого элемента (mediaqueries-5 §1.3): `:root{font-size:
        // 30000px}` на них не влияет (`mq-calc-003`, `mq-calc-004`).
        Some(crate::style::values::value::Len::Ex(k)) => {
            Some(k * crate::text::metrics::ch_ex_px("", 16.0).1)
        }
        Some(crate::style::values::value::Len::Ch(k)) => {
            Some(k * crate::text::metrics::ch_ex_px("", 16.0).0)
        }
        Some(crate::style::values::value::Len::Calc(id)) => {
            let sum = crate::style::values::value::calc_get(id, vcx);
            let rest = crate::style::values::value::Sum {
                px: 0.0,
                em: 0.0,
                ..sum
            };
            if rest == crate::style::values::value::Sum::default() {
                Some(sum.px + sum.em * 16.0)
            } else {
                None
            }
        }
        // Число, в том числе `calc(2 * sign(…))` (css-values-4 §10).
        _ => crate::style::values::value::number(s, vcx),
    }
}

/// Скобки сбалансированы: `a) / calc(b` — это не тело одного `calc()`.
fn balanced(s: &str) -> bool {
    let mut depth = 0i32;
    s.bytes().all(|c| {
        depth += i32::from(c == b'(') - i32::from(c == b')');
        depth >= 0
    }) && depth == 0
}

/// `a / b` по косой ВНЕ скобок: `calc(x / y) / calc(z)` делится на две части.
fn split_top_slash(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, c) in s.bytes().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'/' if depth == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}
