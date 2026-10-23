//! Значения длинных свойств шрифта реестра (css-fonts-4 §2-§7, css-inline-3
//! §line-height): ОДИН разбор на свойство — им реестр проверяет объявление, им
//! же обработчик пишет поля стиля.

use crate::document::state::ValueCx;
use crate::style::computed::{family_name_ok, split_outside_parens};
use crate::style::css::LonghandId;
use crate::style::values::value::Len;

/// Годится ли значение длинному свойству шрифта.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    let v = value.trim();
    match id {
        FontStyle => font_style(v).is_some(),
        FontVariantLigatures | FontVariantCaps | FontVariantNumeric | FontVariantEastAsian
        | FontVariantPosition => variant_tags(id, v).is_some(),
        FontVariantAlternates => crate::text::fonts::alternates::parse(v, false, vcx).is_some(),
        FontVariantEmoji => emoji(v).is_some(),
        FontWeight => font_weight(v, vcx).is_some(),
        FontWidth => font_width(v, vcx).is_some(),
        FontSize => font_size(v, vcx).is_some(),
        LineHeight => line_height(v, vcx).is_some(),
        FontFamily => font_family(v),
        FontKerning => kerning(v).is_some(),
        FontSizeAdjust => size_adjust(v, vcx).is_some(),
        FontFeatureSettings => crate::style::computed::feature_list(v).is_some(),
        FontSynthesisWeight
        | FontSynthesisStyle
        | FontSynthesisSmallCaps
        | FontSynthesisPosition => synthesis(id, v).is_some(),
        _ => false,
    }
}

fn lower(v: &str) -> String {
    v.trim().to_ascii_lowercase()
}

/// `font-style: normal | italic | oblique <angle [-90deg,90deg]>?` (css-fonts-4
/// §font-style-prop): курсив, наклон и угол наклона.
pub(crate) fn font_style(v: &str) -> Option<(bool, bool, Option<f32>)> {
    let v = lower(v);
    let mut parts = v.split_whitespace();
    let kind = parts.next()?;
    let angle = match parts.next() {
        Some(a) if kind == "oblique" => Some(crate::text::fonts::matching::angle(a)?),
        Some(_) => return None,
        None => None,
    };
    if parts.next().is_some() {
        return None;
    }
    match kind {
        "normal" => Some((false, false, None)),
        "italic" => Some((true, false, None)),
        // Голый `oblique` у нас — синтетический наклон курсивом (подбор лица
        // различает их по `oblique`).
        "oblique" => Some((angle.is_none(), true, angle)),
        _ => None,
    }
}

/// `font-weight: normal | bold | bolder | lighter | <number [1,1000]>`
/// (css-fonts-4 §font-weight-prop): число и шаг относительного веса.
pub(crate) fn font_weight(v: &str, vcx: ValueCx<'_>) -> Option<(Option<u16>, i8)> {
    Some(match lower(v).as_str() {
        "normal" => (Some(400), 0),
        "bold" => (Some(700), 0),
        "bolder" => (None, 1),
        "lighter" => (None, -1),
        number => {
            let n = crate::style::values::value::number(number, vcx)?;
            if !(1.0..=1000.0).contains(&n) || number.ends_with('%') {
                return None;
            }
            (Some(n.round() as u16), 0)
        }
    })
}

/// `font-width: normal | <percentage [0,∞]> | ultra-condensed | … |
/// ultra-expanded` (css-fonts-4 §font-width-prop): доля в процентах.
pub(crate) fn font_width(v: &str, vcx: ValueCx<'_>) -> Option<f32> {
    let pct = match lower(v).as_str() {
        "ultra-condensed" => 50.0,
        "extra-condensed" => 62.5,
        "condensed" => 75.0,
        "semi-condensed" => 87.5,
        "normal" => 100.0,
        "semi-expanded" => 112.5,
        "expanded" => 125.0,
        "extra-expanded" => 150.0,
        "ultra-expanded" => 200.0,
        other
            if other.ends_with('%')
                || ["calc(", "min(", "max(", "clamp("]
                    .iter()
                    .any(|f| other.starts_with(f)) =>
        {
            match Len::parse(other, vcx) {
                Some(Len::Pct(p)) if p.is_finite() && p >= 0.0 => p * 100.0,
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(pct)
}

/// `font-family: [ <family-name> | <generic-family> ]#` (css-fonts-4
/// §font-family-prop). Общие слова и `default` без кавычек как имя
/// зарезервированы (§family-name-syntax).
pub(crate) fn font_family(v: &str) -> bool {
    v.split(',').all(|part| {
        let part = part.trim();
        family_name_ok(part)
            && !matches!(
                part.to_ascii_lowercase().as_str(),
                "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
            )
    })
}

/// `font-kerning: auto | normal | none`: 2, 1, 0.
pub(crate) fn kerning(v: &str) -> Option<u8> {
    Some(match lower(v).as_str() {
        "none" => 0,
        "normal" => 1,
        "auto" => 2,
        _ => return None,
    })
}

/// `font-size-adjust: none | [ ex-height | cap-height | ch-width | ic-width |
/// ic-height ]? [ from-font | <number [0,∞]> ]` (css-fonts-5): метрика и
/// число (`NaN` — `from-font`); `none` — метрика `u8::MAX`.
pub(crate) fn size_adjust(v: &str, vcx: ValueCx<'_>) -> Option<(u8, f32)> {
    let lower = lower(v);
    if lower == "none" {
        return Some((u8::MAX, 0.0));
    }
    let toks = split_outside_parens(&lower);
    let (metric, number) = match toks.as_slice() {
        [n] => ("ex-height", n.as_str()),
        [m, n] => (m.as_str(), n.as_str()),
        _ => return None,
    };
    let metric = match metric {
        "ex-height" => 0,
        "cap-height" => 1,
        "ch-width" => 2,
        "ic-width" => 3,
        "ic-height" => 4,
        _ => return None,
    };
    let want = if number == "from-font" {
        f32::NAN
    } else {
        match crate::style::values::value::number(number, vcx) {
            Some(k) if k >= 0.0 => k,
            _ => return None,
        }
    };
    Some((metric, want))
}

/// `font-variant-emoji: normal | text | emoji | unicode` (css-fonts-4 §6.13).
pub(crate) fn emoji(v: &str) -> Option<u8> {
    Some(match lower(v).as_str() {
        "normal" => 0,
        "text" => 1,
        "emoji" => 2,
        "unicode" => 3,
        _ => return None,
    })
}

/// `font-synthesis-*: auto | none` (у `-style` ещё `oblique-only`,
/// css-fonts-4 §font-synthesis-style): 1 — разрешено, 0 — нет, 2 —
/// только наклон.
pub(crate) fn synthesis(id: LonghandId, v: &str) -> Option<u8> {
    Some(match lower(v).as_str() {
        "auto" => 1,
        "none" => 0,
        "oblique-only" if id == LonghandId::FontSynthesisStyle => 2,
        _ => return None,
    })
}

pub(crate) use super::size::{FontSize, font_size, line_height};
pub(crate) use super::variants::{GROUP_TAGS, variant_group, variant_tags};
