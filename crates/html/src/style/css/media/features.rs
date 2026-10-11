//! Признаки медиазапроса: значение признака, булев признак, числа и ключевые слова среды; комбинаторы трёхзначной логики.

use super::*;

impl Media {
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
            "prefers-reduced-motion"
            | "prefers-reduced-transparency"
            | "prefers-reduced-data"
            | "prefers-contrast"
            | "forced-colors" => "no-preference",
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
fn mq_keywords(name: &str) -> Option<&'static [&'static str]> {
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
pub(super) fn mq_base(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix("min-")
        .or_else(|| s.strip_prefix("max-"))
        .unwrap_or(s)
}

fn mq_tri(v: bool) -> SupTri {
    if v { SupTri::True } else { SupTri::False }
}

/// Трёхзначные `and`/`or` (mediaqueries-5 §3.1): «неизвестно» побеждает
/// всё, кроме определяющего исхода.
pub(super) fn mq_and(a: SupTri, b: SupTri) -> SupTri {
    match (a, b) {
        (SupTri::False, _) | (_, SupTri::False) => SupTri::False,
        (SupTri::True, SupTri::True) => SupTri::True,
        _ => SupTri::Unknown,
    }
}

pub(super) fn mq_or(a: SupTri, b: SupTri) -> SupTri {
    match (a, b) {
        (SupTri::True, _) | (_, SupTri::True) => SupTri::True,
        (SupTri::False, SupTri::False) => SupTri::False,
        _ => SupTri::Unknown,
    }
}
