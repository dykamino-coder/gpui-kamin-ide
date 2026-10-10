//! Медиазапросы: Media, сравнения диапазонов, PRINT_MEDIA.

use crate::style::css::*;

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

/// Носитель по умолчанию — печать? Ставит стенд для `*-print`-тестов;
/// приложение всегда экран.
pub static PRINT_MEDIA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Default for Media {
    fn default() -> Self {
        let print = PRINT_MEDIA.load(std::sync::atomic::Ordering::Relaxed);
        // Печатный носитель меряется ЛИСТОМ, а не окном (mediaqueries-4
        // §width: «For paged media, this is the width of the page box»).
        // Лист WPT по умолчанию — 5in x 3in = 480x288, и `@page { size }`
        // запрос не меняет (csswg#5437; `media-queries-001-print`: запрос
        // 4in..5in x 2in..3in при `@page { size: 10in }` обязан сработать).
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

/// Сравнение в диапазонной записи медиа-фичи (mediaqueries-5 §3.3).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MqCmp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

impl MqCmp {
    /// Перевернуть на другую сторону: в `(100px < width)` фича СПРАВА, и
    /// отношение к ней обратное.
    pub(crate) fn flip(self) -> Self {
        match self {
            MqCmp::Lt => MqCmp::Gt,
            MqCmp::Le => MqCmp::Ge,
            MqCmp::Gt => MqCmp::Lt,
            MqCmp::Ge => MqCmp::Le,
            MqCmp::Eq => MqCmp::Eq,
        }
    }

    pub(crate) fn holds(self, a: f32, b: f32) -> bool {
        match self {
            MqCmp::Lt => a < b,
            MqCmp::Le => a <= b,
            MqCmp::Gt => a > b,
            MqCmp::Ge => a >= b,
            MqCmp::Eq => a == b,
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
fn mq_operand(raw: &str) -> Option<(f32, f32)> {
    let s = raw.trim();
    // `calc(a / b)` — отношение внутри calc (`mq-calc-008`).
    let body = s
        .strip_prefix("calc(")
        .and_then(|r| r.strip_suffix(')'))
        .unwrap_or(s);
    if let Some((a, b)) = body.split_once('/')
        && let (Some(a), Some(b)) = (mq_scalar(a), mq_scalar(b))
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
    mq_scalar(s).map(|v| (v, 1.0))
}

/// Скаляр: длина в точках, разрешение в dppx или голое число.
fn mq_scalar(raw: &str) -> Option<f32> {
    let s = raw.trim();
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
    if let Some(n) = s.strip_suffix("dpi") {
        return n.trim().parse::<f32>().ok().map(|v| v / 96.0);
    }
    if let Some(n) = s.strip_suffix("dpcm") {
        return n.trim().parse::<f32>().ok().map(|v| v * 2.54 / 96.0);
    }
    match crate::style::values::value::Len::parse(s) {
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
            let sum = crate::style::values::value::calc_get(id);
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
        _ => s.parse::<f32>().ok(),
    }
}

impl Media {
    /// Выполняется ли условие `@media` — mediaqueries-5 §3 «Media Queries»
    /// и §3.1 «Error Handling».
    ///
    /// Логика ТРЁХЗНАЧНАЯ. Нераспознанная скобка — это `<general-enclosed>`,
    /// и её значение «неизвестно», а не «ложь»: `not (unknown)` обязано
    /// остаться «неизвестно» (то есть на верхнем уровне ложью), тогда как
    /// прежний двухзначный разбор переворачивал ложь в истину и применял
    /// правило (`at-media-002`, `negation-001`, `mq-gamut-003`).
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim();
        let q = q.strip_prefix("@media").unwrap_or(q).trim();
        // Пустой список запросов равен `all` (mediaqueries-5 §2.1:
        // «An empty media query list evaluates to true»): пробел после
        // `@media` необязателен, и `@media{ ... }` обязано примениться
        // (`at-media-whitespace-optional-001/002`).
        if q.is_empty() {
            return true;
        }
        // Список запросов через запятую — «или». Негодный запрос НЕ валит
        // список целиком (§3.1: он заменяется на `not all`), поэтому каждая
        // часть считается сама по себе.
        split_top_level(q, ',')
            .into_iter()
            .any(|alt| self.mq_query(alt) == SupTri::True)
    }
}
