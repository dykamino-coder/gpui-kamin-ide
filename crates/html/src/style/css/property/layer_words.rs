//! Слова слоя фона и маски для раскрытия сокращений: деление слоя на слова
//! (`/` — отдельным словом, ключевые слова — строчными, css-values-4 §2.1:
//! ключевые слова регистронезависимы) и узнавание частей слоя. С документом
//! части узнаёт разбор их значений; без него (CSS самого движка) — форма
//! слова.

use crate::document::state::ValueCx;

/// Узнавание частей слоя.
pub(crate) struct Words<'v> {
    pub vcx: Option<ValueCx<'v>>,
}

const POSITION_WORDS: &[&str] = &["left", "right", "top", "bottom", "center"];

fn math_start(w: &str) -> bool {
    ["calc(", "min(", "max(", "clamp(", "var("]
        .iter()
        .any(|f| w.starts_with(f))
}

fn numeric(w: &str) -> bool {
    w.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')) || math_start(w)
}

impl Words<'_> {
    /// Слово `<bg-position>`/`<position>`: ключевое слово оси или
    /// `<length-percentage>`.
    pub fn position(&self, w: &str) -> bool {
        POSITION_WORDS.contains(&w) || self.length(w)
    }

    /// `<length-percentage>` (с функциями сравнения).
    pub fn length(&self, w: &str) -> bool {
        match self.vcx {
            Some(vcx) => crate::style::computed::Computed::background_position_length(w, vcx),
            None => numeric(w),
        }
    }

    /// Слово `<bg-size>`.
    pub fn size(&self, w: &str) -> bool {
        matches!(w, "auto" | "cover" | "contain") || self.length(w)
    }

    /// `none | <image>` (у маски — и `<mask-source>`, тот же `url()`).
    pub fn image(&self, w: &str) -> bool {
        if w == "none" {
            return true;
        }
        match self.vcx {
            Some(vcx) => crate::style::computed::ImageSource::parse(w, vcx).is_some(),
            None => {
                crate::style::css::at_url(w) || {
                    let w = w.to_ascii_lowercase();
                    [
                        "image(",
                        "image-set(",
                        "-webkit-image-set(",
                        "cross-fade(",
                        "element(",
                        "paint(",
                        "filter(",
                        "light-dark(",
                    ]
                    .iter()
                    .any(|f| w.starts_with(f))
                        || w.contains("gradient(")
                }
            }
        }
    }

    /// `<color>`.
    pub fn color(&self, w: &str) -> bool {
        match self.vcx {
            Some(vcx) => crate::style::values::value::StyleColor::parse_value(w, vcx).is_some(),
            None => !w.contains('/'),
        }
    }
}

/// Слои значения-списка через запятую (`<…>#`); пустой слой (`a,,b`,
/// `a,`) делает значение негодным.
pub(crate) fn layer_list(v: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = crate::style::css::split_top_level(v, ',')
        .into_iter()
        .map(str::trim)
        .collect();
    parts.iter().all(|p| !p.is_empty()).then_some(parts)
}

/// Слова слоя вне скобок; `/` вне скобок — отдельное слово; ключевые
/// слова приводятся к строчным.
pub(crate) fn layer_tokens(layer: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(layer.len() + 4);
    let mut depth = 0usize;
    for ch in layer.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '/' if depth == 0 => {
                spaced.push_str(" / ");
                continue;
            }
            _ => {}
        }
        spaced.push(ch);
    }
    crate::style::computed::split_outside_parens(&spaced)
        .into_iter()
        .map(|w| {
            if w.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
                w.to_ascii_lowercase()
            } else {
                w
            }
        })
        .collect()
}

/// `<bg-size>` после `/` с места `start`: `cover`, `contain` или одно-два
/// слова `auto | <length-percentage>`. Текст размера и место за ним.
pub(crate) fn size_run(t: &[String], start: usize, w: &Words<'_>) -> Option<(String, usize)> {
    let first = t.get(start)?;
    if matches!(first.as_str(), "cover" | "contain") {
        return Some((first.clone(), start + 1));
    }
    let end = (start..t.len().min(start + 2))
        .take_while(|&j| t[j] != "cover" && t[j] != "contain" && w.size(&t[j]))
        .last()?
        + 1;
    Some((t[start..end].join(" "), end))
}
