//! Сокращение `font`, раскрытое при разборе (css-fonts-4 §font-prop, с
//! системными семействами и сбросом подсвойств). Части идут на места
//! `ShorthandId::longhands`; опущенная часть — начальное значение. Порядок
//! головы `font` и разбор — как Blink `ConsumeFont`: до четырёх слов начертания,
//! варианта CSS 2, веса и ширины CSS 3 в любом порядке (`normal` годится
//! любому), затем кегль, `/ высота строки` и список семейств.

use super::LonghandId;
use crate::document::state::ValueCx;
use LonghandId::*;

pub(super) const FONT: &[LonghandId] = &[
    FontStyle,
    FontVariantCaps,
    FontWeight,
    FontWidth,
    FontSize,
    LineHeight,
    FontFamily,
    // Сбрасываемые подсвойства (§font-prop «Reset Implicitly»).
    FontVariantLigatures,
    FontVariantAlternates,
    FontVariantNumeric,
    FontVariantEastAsian,
    FontVariantPosition,
    FontVariantEmoji,
    FontKerning,
    FontSizeAdjust,
    FontFeatureSettings,
];

/// Слова значения верхнего уровня с их местом в тексте: пробелы и `/` вне
/// скобок и строк делят слова, `/` — отдельное слово.
pub(super) fn words(v: &str) -> Option<Vec<(usize, usize)>> {
    let (mut out, mut start, mut depth, mut quote) = (Vec::new(), None, 0i32, None);
    for (i, ch) in v.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                start.get_or_insert(i);
            }
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => depth -= 1,
            '/' if depth == 0 => {
                if let Some(s) = start.take() {
                    out.push((s, i));
                }
                out.push((i, i + 1));
            }
            c if c.is_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    out.push((s, i));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if quote.is_some() || depth != 0 {
        return None;
    }
    if let Some(s) = start {
        out.push((s, v.len()));
    }
    Some(out)
}

/// Системное семейство (css-fonts-4 §system-family-name) — его кегль и
/// семейство на этой платформе. Как Blink `LayoutThemeFontProviderWin`:
/// `caption`, `icon`, `message-box` — шрифт интерфейса по умолчанию (Arial)
/// кеглем документа по умолчанию; `menu`, `small-caption`, `status-bar` —
/// шрифты `NONCLIENTMETRICS` Windows (Segoe UI 9pt).
fn system_font(word: &str) -> Option<(&'static str, &'static str)> {
    Some(match word {
        "caption" | "icon" | "message-box" => ("16px", "Arial"),
        "menu" | "small-caption" | "status-bar" => ("12px", "\"Segoe UI\""),
        _ => return None,
    })
}

const WIDTH_CSS3: &[&str] = &[
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
];

/// Вес головы `font`: слово или `<number [1,1000]>`.
fn weight(word: &str, vcx: Option<ValueCx<'_>>) -> bool {
    matches!(word, "bold" | "bolder" | "lighter")
        || (!word.ends_with('%')
            && match vcx {
                Some(vcx) => crate::style::values::value::number(word, vcx),
                None => word.parse::<f32>().ok(),
            }
            .is_some_and(|n| (1.0..=1000.0).contains(&n)))
}

/// `font`: части по [`FONT`].
pub(super) fn font(value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
    let v = value.trim();
    let spans = words(v)?;
    let word = |i: usize| spans.get(i).map(|&(s, e)| &v[s..e]);
    let mut parts: Vec<String> = FONT.iter().map(|id| id.initial().to_string()).collect();
    if spans.len() == 1
        && let Some((size, family)) = system_font(&v.to_ascii_lowercase())
    {
        parts[4] = size.into();
        parts[6] = family.into();
        return Some(parts);
    }
    let (mut style, mut caps, mut weight_set, mut width) = (false, false, false, false);
    let mut i = 0;
    for _ in 0..4 {
        let Some(w) = word(i) else { break };
        let lower = w.to_ascii_lowercase();
        match lower.as_str() {
            "normal" => {}
            "italic" | "oblique" if !style => {
                style = true;
                parts[0] = lower.clone();
                // `oblique <angle>` — угол сразу за словом (§font-style-prop).
                if lower == "oblique"
                    && let Some(a) = word(i + 1)
                    && crate::font_facts::matching::angle(&a.to_ascii_lowercase()).is_some()
                {
                    parts[0] = format!("oblique {a}");
                    i += 1;
                }
            }
            "small-caps" if !caps => {
                caps = true;
                parts[1] = lower.clone();
            }
            w if !width && WIDTH_CSS3.contains(&w) => {
                width = true;
                parts[3] = lower.clone();
            }
            w if !weight_set && weight(w, vcx) => {
                weight_set = true;
                parts[2] = lower.clone();
            }
            _ => break,
        }
        i += 1;
    }
    parts[4] = word(i)?.to_string();
    i += 1;
    if word(i) == Some("/") {
        parts[5] = word(i + 1)?.to_string();
        i += 2;
    }
    let family = v[spans.get(i)?.0..].trim();
    if family.is_empty() || family.ends_with(',') {
        return None;
    }
    parts[6] = family.to_string();
    Some(parts)
}
