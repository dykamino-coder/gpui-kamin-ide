//! Представленческие атрибуты HTML на границе DOM и каскада.
//! `rules` собирает для элемента правила происхождения «author presentational
//! hint» (css-cascade-5 §6.4: ниже всех авторских слоёв, выше листа агента):
//! `nowrap` ячеек с quirks-исключением, `cellpadding` ближайшей таблицы, `cellspacing` таблицы,
//! `height` таблицы, `width` колонок, `color` у `<font>`, подсказки `<hr>`
//! (`presentational_hr`), таблиц/списков/вложений (`presentational_legacy`) и
//! атрибуты `<foreignObject>` (SVG 2 §6.6). `colors` — `bgcolor`/`text`
//! в стиль; `dimension` — разбор размерного значения (HTML §2.3.4.4).

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::{Rule, Selector, parse_decls};

mod svg;
use svg::foreign_object;

/// HTML rendering §15.3.8: cell nowrap is a presentational hint, with a
/// nonzero-length width exception in quirks mode. Author CSS takes precedence.
pub(super) fn nowrap(tag: &str, attrs: &[(String, String)], quirks: bool) -> Option<Rule> {
    if !matches!(tag, "td" | "th") || !attrs.iter().any(|(k, _)| k == "nowrap") {
        return None;
    }
    let normal = quirks && attrs.iter().any(|(k, v)| k == "width" && nonzero_length(v));
    hint(if normal {
        "white-space: normal"
    } else {
        "white-space: nowrap"
    })
}

pub(super) fn hint(css: &str) -> Option<Rule> {
    Some(Rule {
        sel: Selector::parse("*")?,
        decls: parse_decls(css),
        order: 0,
        origin: crate::style::css::Origin::PresHint,
        // Below every author layer, above UA rules; part of author for revert.
        // CSS Cascade 5 §6.4: author presentational hint origin.
        layer: Vec::new(),
    })
}

/// HTML §15.3.8 maps table height to the author presentational-hint origin.
/// Keep it in the cascade so auto, initial, inherit and authored lengths win.
pub(super) fn rules(
    tag: &str,
    attrs: &[(String, String)],
    path: &[crate::style::select::Ancestor],
    vcx: ValueCx<'_>,
) -> Vec<Rule> {
    let quirks = vcx.doc.quirks();
    nowrap(tag, attrs, quirks)
        .into_iter()
        .chain(cell_padding(tag, path))
        .chain(cell_spacing(tag, attrs))
        .chain(table_height(tag, attrs))
        .chain(column_width(tag, attrs))
        .chain(font_color(tag, attrs, vcx))
        .chain(super::presentational_hr::rule(tag, attrs, vcx))
        .chain(super::presentational_legacy::rule(tag, attrs, path, vcx))
        .chain(foreign_object(tag, attrs))
        .chain(svg::path_length(tag, attrs))
        .collect()
}

/// HTML rendering §15.3.4: legacy font colors participate below author CSS.
fn font_color(tag: &str, attrs: &[(String, String)], vcx: ValueCx<'_>) -> Option<Rule> {
    if tag != "font" {
        return None;
    }
    let raw = &attrs.iter().find(|(name, _)| name == "color")?.1;
    let color = super::presentational_hr::legacy_color(raw, vcx)?;
    hint(&format!("color: {color}"))
}

/// HTML rendering §15.3.8 maps col/colgroup width into the hint origin.
/// Servo Element::synthesize_presentational_hints_for_legacy_attributes
/// obtains HTMLTableColElement::get_width before the author cascade.
fn column_width(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if !matches!(tag, "col" | "colgroup") {
        return None;
    }
    let raw = &attrs.iter().find(|(name, _)| name == "width")?.1;
    let (value, percent) = dimension(raw)?;
    hint(&format!(
        "width: {value}{}",
        if percent { "%" } else { "px" }
    ))
}

/// HTML rendering §15.3.8: `cellpadding` of the nearest table ancestor is a
/// pixel presentational hint for `padding-*` of td/th (non-negative integer,
/// §2.3.4.1). In the hint origin it also feeds column measurement, not only
/// paint. Blink: `HTMLTableElement::CellPadding`; Servo `get_cellpadding`.
fn cell_padding(tag: &str, path: &[crate::style::select::Ancestor]) -> Option<Rule> {
    if !matches!(tag, "td" | "th") {
        return None;
    }
    let table = path.iter().rev().find(|a| a.tag == "table")?;
    let raw = &table
        .attrs
        .iter()
        .find(|(name, _)| name == "cellpadding")?
        .1;
    let px = super::presentational_hr::non_negative_integer(raw)?;
    hint(&format!("padding: {px}px"))
}

/// HTML rendering §15.3.8: `cellspacing` задаёт пиксельный `border-spacing`
/// в происхождении подсказок, где авторские объявления имеют приоритет.
fn cell_spacing(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if tag != "table" {
        return None;
    }
    let raw = &attrs.iter().find(|(name, _)| name == "cellspacing")?.1;
    let px = super::presentational_hr::non_negative_integer(raw)?;
    hint(&format!("border-spacing: {px}px"))
}

fn table_height(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if tag != "table" {
        return None;
    }
    let raw = &attrs.iter().find(|(name, _)| name == "height")?.1;
    let (value, percent) = dimension(raw)?;
    hint(&format!(
        "height: {value}{}",
        if percent { "%" } else { "px" }
    ))
}

/// HTML §2.3.4.4 accepts a nonnegative decimal prefix, including zero.
/// A percent sign must immediately follow that prefix; other suffixes are ignored.
pub(super) fn dimension(raw: &str) -> Option<(f32, bool)> {
    let raw = raw.trim_start_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let bytes = raw.as_bytes();
    let mut end = 0;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    let value: f32 = raw[..end].parse().ok()?;
    value
        .is_finite()
        .then_some((value, bytes.get(end) == Some(&b'%')))
}

/// HTML §2.3.4.4–5: a decimal numeric prefix, with % recognized immediately
/// after that prefix. Zero, a leading sign and a missing integer are invalid.
fn nonzero_length(raw: &str) -> bool {
    let bytes = raw
        .trim_start_matches(['\t', '\n', '\u{c}', '\r', ' '])
        .as_bytes();
    let mut i = 0;
    let mut nonzero = false;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        nonzero |= bytes[i] != b'0';
        i += 1;
    }
    if i == 0 {
        return false;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            nonzero |= bytes[i] != b'0';
            i += 1;
        }
    }
    nonzero && bytes.get(i) != Some(&b'%')
}

/// Preserve the existing bgcolor/text fallback below authored colors.
pub(super) fn colors(
    style: &mut Computed,
    tag: &str,
    attrs: &[(String, String)],
    vcx: ValueCx<'_>,
) {
    let color_of = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| super::presentational_hr::legacy_color(v, vcx))
            .and_then(|v| crate::style::values::value::Color::parse(&v, vcx))
    };
    if matches!(tag, "body" | "table" | "tr" | "td" | "th")
        && style.background.is_none()
        && style.gradient.is_none()
        && let Some(c) = color_of("bgcolor")
    {
        style.set_background_color(Some(c.into()));
    }
    if tag == "body"
        && style.color.is_none()
        && let Some(c) = color_of("text")
    {
        style.color = Some(c);
    }
}

#[cfg(test)]
mod tests;
