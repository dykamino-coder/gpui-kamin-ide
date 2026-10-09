//! Медиазапросы: Media, сравнения диапазонов, PRINT_MEDIA.

use crate::style::css::*;

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
        let (width, height) = if print { (480.0, 288.0) } else { (1280.0, 800.0) };
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
pub(crate) fn take_parens(s: &str) -> Option<(&str, &str)> {
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
pub(crate) fn mq_operand(raw: &str) -> Option<(f32, f32)> {
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
pub(crate) fn mq_scalar(raw: &str) -> Option<f32> {
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
        Some(crate::style::values::value::Len::Ex(k)) => Some(k * crate::text::metrics::ch_ex_px("", 16.0).1),
        Some(crate::style::values::value::Len::Ch(k)) => Some(k * crate::text::metrics::ch_ex_px("", 16.0).0),
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

    /// Один `<media-query>`: `[not | only]? <media-type> [and <cond>]?`
    /// либо голое `<media-condition>`.
    pub(crate) fn mq_query(&self, raw: &str) -> SupTri {
        let s = raw.trim().to_ascii_lowercase();
        if s.is_empty() {
            return SupTri::False;
        }
        // Голое условие узнаётся по скобке: `<media-condition>` всегда
        // начинается со скобки либо с `not (`.
        if s.starts_with('(') || s.starts_with("not (") {
            return self.mq_condition(&s);
        }
        let (negate, rest) = if let Some(r) = s.strip_prefix("not ") {
            (true, r.trim())
        } else if let Some(r) = s.strip_prefix("only ") {
            (false, r.trim())
        } else {
            (false, s.as_str())
        };
        let (ty, tail) = match rest.split_once(" and ") {
            Some((t, c)) => (t.trim(), Some(c.trim())),
            None => (rest.trim(), None),
        };
        // `<media-type>` — идентификатор, и слова `only`/`not`/`and`/`or`/
        // `layer` типом БЫТЬ НЕ МОГУТ (mediaqueries-5 §3). Значит `@media and`
        // и `@media not and` — синтаксические ошибки, а ошибочный запрос
        // равен `not all`, то есть ЛОЖЬ даже под отрицанием
        // (`mq-invalid-media-type-001..004`, `-layer-001`).
        if ty.is_empty()
            || matches!(ty, "only" | "not" | "and" | "or" | "layer")
            || !ty.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return SupTri::False;
        }
        let hit = match ty {
            "all" => SupTri::True,
            "screen" if !self.print => SupTri::True,
            "print" if self.print => SupTri::True,
            // Неизвестный, но грамматически годный тип — ЛОЖЬ, а не ошибка:
            // `not unknown` истинно (`at-media-002`).
            _ => SupTri::False,
        };
        // Отрицание относится ко всему `<media-type> and <cond>`, а не к
        // одному типу, поэтому склейка идёт первой.
        let full = match tail {
            None => hit,
            Some(cond) => mq_and(hit, self.mq_condition(cond)),
        };
        if negate { sup_not(full) } else { full }
    }

    /// `<media-condition>`: `not <in-parens>` | `<in-parens> (and ..)*` |
    /// `<in-parens> (or ..)*`. Смешивать `and` и `or` без скобок нельзя
    /// (§3.1), такой запрос негоден целиком.
    pub(crate) fn mq_condition(&self, raw: &str) -> SupTri {
        let s = raw.trim();
        if let Some(rest) = s.strip_prefix("not ") {
            let Some((term, tail)) = take_parens(rest.trim()) else {
                return SupTri::False;
            };
            if !tail.trim().is_empty() {
                return SupTri::False;
            }
            return sup_not(self.mq_in_parens(term));
        }
        let Some((term, mut rest)) = take_parens(s) else {
            return SupTri::False;
        };
        let mut acc = self.mq_in_parens(term);
        let mut op: Option<&str> = None;
        loop {
            let r = rest.trim_start();
            if r.is_empty() {
                return acc;
            }
            let word = if let Some(w) = r.strip_prefix("and ") {
                rest = w;
                "and"
            } else if let Some(w) = r.strip_prefix("or ") {
                rest = w;
                "or"
            } else {
                return SupTri::False;
            };
            if *op.get_or_insert(word) != word {
                return SupTri::False;
            }
            let Some((term, tail)) = take_parens(rest.trim_start()) else {
                return SupTri::False;
            };
            let v = self.mq_in_parens(term);
            acc = if word == "and" {
                mq_and(acc, v)
            } else {
                mq_or(acc, v)
            };
            rest = tail;
        }
    }

    /// `<media-in-parens>` без внешних скобок: вложенное условие, фича или
    /// `<general-enclosed>`.
    pub(crate) fn mq_in_parens(&self, inner: &str) -> SupTri {
        let s = inner.trim();
        if s.starts_with('(') || s.starts_with("not ") {
            return self.mq_condition(s);
        }
        // Диапазонная запись §3.3: `(width > 100px)`, `(100px <= width)`,
        // `(100px < width < 200px)`. Двусторонняя форма — это И двух
        // односторонних.
        if s.contains('<') || s.contains('>') || (s.contains('=') && !s.contains(':')) {
            return self.mq_range(s);
        }
        if let Some((name, value)) = s.split_once(':') {
            return self.mq_feature(name.trim(), value.trim(), MqCmp::Eq);
        }
        // Булева форма `<mf-boolean>`: голое имя истинно, когда значение
        // фичи не «ноль/none» (§2.4.2).
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return self.mq_boolean(s);
        }
        // `<general-enclosed>` — «неизвестно» (§3.1): пустые скобки, мусор,
        // сломанный диапазон.
        SupTri::Unknown
    }

    /// Диапазонная запись. Разрез по знакам сравнения; фича может стоять
    /// слева, справа либо между двумя пределами.
    pub(crate) fn mq_range(&self, s: &str) -> SupTri {
        let mut parts: Vec<(&str, MqCmp)> = vec![];
        let mut start = 0usize;
        let b = s.as_bytes();
        let mut at = 0usize;
        while at < b.len() {
            let (cmp, len) = match (b[at], b.get(at + 1)) {
                (b'<', Some(b'=')) => (MqCmp::Le, 2),
                (b'>', Some(b'=')) => (MqCmp::Ge, 2),
                (b'<', _) => (MqCmp::Lt, 1),
                (b'>', _) => (MqCmp::Gt, 1),
                (b'=', _) => (MqCmp::Eq, 1),
                _ => {
                    at += 1;
                    continue;
                }
            };
            parts.push((&s[start..at], cmp));
            at += len;
            start = at;
        }
        // Ни одного знака — это не диапазон, а мусор (`mq-range-001`:
        // `(width 500px)` без знака НЕгоден и обязан стать
        // `<general-enclosed>`, а не ошибкой запроса).
        if parts.is_empty() {
            return SupTri::Unknown;
        }
        let tail = s[start..].trim();
        match parts.len() {
            1 => {
                let (left, cmp) = parts[0];
                let left = left.trim();
                // Фича слева — сравнение как есть; фича справа — обратное.
                if self.mq_numeric(mq_base(left)).is_some() {
                    self.mq_feature(left, tail, cmp)
                } else {
                    self.mq_feature(tail, left, cmp.flip())
                }
            }
            2 => {
                let (lo, c1) = parts[0];
                let (name, c2) = parts[1];
                let a = self.mq_feature(name.trim(), lo.trim(), c1.flip());
                let b = self.mq_feature(name.trim(), tail, c2);
                mq_and(a, b)
            }
            _ => SupTri::Unknown,
        }
    }

    /// `<mf-plain>` и разложенные диапазоны. `min-`/`max-` — те же `>=`/`<=`
    /// (§2.4.1); отрицательные пределы разрешены и просто дают ложь или
    /// истину, а НЕ ошибку (§2.4.1 «false in the negative range»,
    /// `mq-negative-range-001/002`).
    pub(crate) fn mq_feature(&self, name: &str, value: &str, cmp: MqCmp) -> SupTri {
        let name = name.trim();
        let (base, cmp) = if let Some(b) = name.strip_prefix("min-") {
            (b, MqCmp::Ge)
        } else if let Some(b) = name.strip_prefix("max-") {
            (b, MqCmp::Le)
        } else {
            (name, cmp)
        };
        // Перечислимые фичи: значение — ключевое слово из закрытого списка.
        // Слово ВНЕ списка делает скобку `<general-enclosed>`, то есть
        // «неизвестно», а не ложь: `not (color-gamut: dci-p3)` обязано
        // остаться ложью на верхнем уровне (`mq-gamut-003`).
        if let Some(allowed) = mq_keywords(base) {
            if !allowed.contains(&value) {
                return SupTri::Unknown;
            }
            return mq_tri(value == self.mq_keyword(base));
        }
        let Some((num, den)) = self.mq_numeric(base) else {
            return SupTri::Unknown;
        };
        let Some((a, b)) = mq_operand(value) else {
            return SupTri::Unknown;
        };
        // Перекрёстное умножение вместо деления: вырожденное `0/0` иначе
        // даёт NaN и валит любое сравнение (Blink, `CompareAspectRatioValue`).
        mq_tri(cmp.holds(num * b, den * a))
    }

    /// Булева форма: фича истинна, когда её значение не ноль и не `none`.
    pub(crate) fn mq_boolean(&self, name: &str) -> SupTri {
        if mq_keywords(name).is_some() {
            return mq_tri(!matches!(self.mq_keyword(name), "none" | "no-preference"));
        }
        match self.mq_numeric(name) {
            Some((num, den)) => mq_tri(num != 0.0 && den != 0.0),
            None => SupTri::Unknown,
        }
    }

    /// Числовое значение фичи этого носителя как отношение (числитель,
    /// знаменатель). `None` — фича не наша, то есть `<general-enclosed>`.
    pub(crate) fn mq_numeric(&self, name: &str) -> Option<(f32, f32)> {
        Some(match name {
            "width" | "device-width" => (self.width, 1.0),
            "height" | "device-height" => (self.height, 1.0),
            "aspect-ratio" | "device-aspect-ratio" => (self.width, self.height),
            "resolution" => (1.0, 1.0),
            "color" => (8.0, 1.0),
            "color-index" | "monochrome" | "grid" => (0.0, 1.0),
            _ => return None,
        })
    }

    /// Значение перечислимой фичи для этого носителя.
    pub(crate) fn mq_keyword(&self, name: &str) -> &'static str {
        match name {
            "orientation" => {
                if self.width >= self.height {
                    "landscape"
                } else {
                    "portrait"
                }
            }
            "scan" => "progressive",
            "update" => "fast",
            "overflow-block" => {
                if self.print {
                    "paged"
                } else {
                    "scroll"
                }
            }
            "overflow-inline" => {
                if self.print {
                    "none"
                } else {
                    "scroll"
                }
            }
            "hover" | "any-hover" => "hover",
            "pointer" | "any-pointer" => "fine",
            "color-gamut" => "srgb",
            "dynamic-range" | "video-dynamic-range" => "standard",
            "prefers-reduced-motion" | "prefers-reduced-transparency"
            | "prefers-reduced-data" | "prefers-contrast" | "forced-colors" => "no-preference",
            "prefers-color-scheme" => {
                if self.dark {
                    "dark"
                } else {
                    "light"
                }
            }
            _ => "",
        }
    }
}

/// Закрытый список значений перечислимой фичи (mediaqueries-5 §4-§11).
pub(crate) fn mq_keywords(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "orientation" => &["portrait", "landscape"],
        "scan" => &["interlace", "progressive"],
        "update" => &["none", "slow", "fast"],
        "overflow-block" => &["none", "scroll", "optional-paged", "paged"],
        "overflow-inline" => &["none", "scroll"],
        "hover" | "any-hover" => &["none", "hover"],
        "pointer" | "any-pointer" => &["none", "coarse", "fine"],
        "color-gamut" => &["srgb", "p3", "rec2020"],
        "dynamic-range" | "video-dynamic-range" => &["standard", "high"],
        "prefers-color-scheme" => &["light", "dark"],
        "prefers-reduced-motion" | "prefers-reduced-transparency" | "prefers-reduced-data" => {
            &["no-preference", "reduce"]
        }
        "prefers-contrast" => &["no-preference", "less", "more", "custom"],
        "forced-colors" => &["none", "active"],
        _ => return None,
    })
}

/// Имя фичи без приставки диапазона — для проверки «фича это или предел».
pub(crate) fn mq_base(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix("min-").or_else(|| s.strip_prefix("max-")).unwrap_or(s)
}

pub(crate) fn mq_tri(v: bool) -> SupTri {
    if v { SupTri::True } else { SupTri::False }
}

/// Трёхзначные `and`/`or` (mediaqueries-5 §3.1): «неизвестно» побеждает
/// всё, кроме определяющего исхода.
pub(crate) fn mq_and(a: SupTri, b: SupTri) -> SupTri {
    match (a, b) {
        (SupTri::False, _) | (_, SupTri::False) => SupTri::False,
        (SupTri::True, SupTri::True) => SupTri::True,
        _ => SupTri::Unknown,
    }
}

pub(crate) fn mq_or(a: SupTri, b: SupTri) -> SupTri {
    match (a, b) {
        (SupTri::True, _) | (_, SupTri::True) => SupTri::True,
        (SupTri::False, SupTri::False) => SupTri::False,
        _ => SupTri::Unknown,
    }
}
