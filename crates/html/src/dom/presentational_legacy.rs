//! HTML rendering §15: представленческие подсказки таблиц, списков, `br` и
//! вложенного содержимого (§15.3.8 таблицы, §15.3.7 списки, §15.3.4 `br`,
//! §15.4.3 атрибуты вложений и картинок).
//!
//! Servo выражает селекторную часть листом `presentational-hints.css`; здесь
//! каждая подсказка считается по собственным атрибутам элемента (у ячеек — и
//! по владеющей таблице) и ложится в происхождение подсказок через
//! [`super::presentational_hints::hint`], ниже всех авторских правил.

use crate::document::state::ValueCx;
use crate::style::css::Rule;
use crate::style::select::Ancestor;

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn lower(attrs: &[(String, String)], name: &str) -> Option<String> {
    attr(attrs, name).map(|v| v.trim().to_ascii_lowercase())
}

/// §2.3.4.4 dimension value as CSS text; `nonzero` drops zero ("ignoring zero").
fn dim(attrs: &[(String, String)], name: &str, nonzero: bool) -> Option<String> {
    let (v, pct) = attr(attrs, name).and_then(super::presentational_hints::dimension)?;
    (!nonzero || v > 0.0).then(|| format!("{v}{}", if pct { "%" } else { "px" }))
}

/// §15.4.3: `img`, `object`, `input type=image` — a `border` greater than zero
/// sets four border widths and four solid border styles.
fn image_border(attrs: &[(String, String)], css: &mut String) {
    if let Some(n) = attr(attrs, "border")
        .and_then(super::presentational_hr::non_negative_integer)
        .filter(|n| *n > 0)
    {
        css.push_str(&format!("border-width: {n}px; border-style: solid;"));
    }
}

// Not mapped yet (narrowed after measurement, see the commit): `table border`
// (§15.3.8 widths + outset table / inset cell edges) — its cell edges expose a
// one-row background offset in bordered vertical-lr tables
// (`table-cell-valign-001/002/003`).

fn table(attrs: &[(String, String)], css: &mut String, vcx: ValueCx<'_>) {
    match lower(attrs, "align").as_deref() {
        Some("left") => css.push_str("float: left;"),
        Some("right") => css.push_str("float: right;"),
        Some("center") => css.push_str("margin-left: auto; margin-right: auto;"),
        _ => {}
    }
    // §15.3.8: `width` is a dimension property ignoring zero, `hspace`/`vspace`
    // map to the inline/block margins.
    if let Some(w) = dim(attrs, "width", true) {
        css.push_str(&format!("width: {w};"));
    }
    if let Some(h) = dim(attrs, "hspace", false) {
        css.push_str(&format!("margin-left: {h}; margin-right: {h};"));
    }
    if let Some(v) = dim(attrs, "vspace", false) {
        css.push_str(&format!("margin-top: {v}; margin-bottom: {v};"));
    }
    if let Some(c) =
        attr(attrs, "bordercolor").and_then(|c| super::presentational_hr::legacy_color(c, vcx))
    {
        css.push_str(&format!("border-color: {c};"));
    }
}

fn list_type(tag: &str, attrs: &[(String, String)], css: &mut String) {
    let Some(raw) = attr(attrs, "type") else {
        return;
    };
    // `ol`/`li` letter and roman types are case-sensitive (`[type=a s]`).
    let ordered = match raw {
        "1" => Some("decimal"),
        "a" => Some("lower-alpha"),
        "A" => Some("upper-alpha"),
        "i" => Some("lower-roman"),
        "I" => Some("upper-roman"),
        _ => None,
    };
    let bullet = match raw.to_ascii_lowercase().as_str() {
        "none" => Some("none"),
        "disc" => Some("disc"),
        "circle" => Some("circle"),
        "square" => Some("square"),
        _ => None,
    };
    let value = match tag {
        "ol" => ordered,
        "ul" => bullet,
        _ => ordered.or(bullet),
    };
    if let Some(v) = value {
        css.push_str(&format!("list-style-type: {v};"));
    }
}

fn embedded_align(attrs: &[(String, String)], css: &mut String) {
    let decl = match lower(attrs, "align").as_deref() {
        Some("left") => "float: left;",
        Some("right") => "float: right;",
        Some("top") => "vertical-align: top;",
        Some("baseline") => "vertical-align: baseline;",
        Some("texttop") => "vertical-align: text-top;",
        Some("absmiddle") | Some("abscenter") => "vertical-align: middle;",
        Some("bottom") => "vertical-align: bottom;",
        _ => "",
    };
    css.push_str(decl);
}

fn valign(attrs: &[(String, String)], css: &mut String) {
    if let Some(v) = lower(attrs, "valign")
        && matches!(v.as_str(), "top" | "middle" | "bottom" | "baseline")
    {
        css.push_str(&format!("vertical-align: {v};"));
    }
}

/// HTML §15.3.3: `align` of block/table-part elements maps to `text-align`
/// (centering of block descendants is not mapped).
fn text_align(tag: &str, attrs: &[(String, String)], css: &mut String) {
    let value = match lower(attrs, "align").as_deref() {
        Some("left") => "left",
        Some("right") => "right",
        Some("justify") => "justify",
        Some("center") | Some("middle") if tag != "p" && !tag.starts_with('h') => "center",
        Some("center") => "center",
        _ => return,
    };
    css.push_str(&format!("text-align: {value};"));
}

/// The hint declarations of one element, as one declaration block.
pub(super) fn hints(
    tag: &str,
    attrs: &[(String, String)],
    _path: &[Ancestor],
    vcx: ValueCx<'_>,
) -> String {
    let mut css = String::new();
    if matches!(
        tag,
        "div"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "td"
            | "th"
            | "p"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    ) {
        text_align(tag, attrs, &mut css);
    }
    match tag {
        "br" => match attr(attrs, "clear").map(str::to_ascii_lowercase).as_deref() {
            Some("left") => css.push_str("clear: left;"),
            Some("right") => css.push_str("clear: right;"),
            Some("all" | "both") => css.push_str("clear: both;"),
            _ => {}
        },
        "table" => table(attrs, &mut css, vcx),
        "td" | "th" => {
            // §15.3.8: cell `width` ignores zero, cell and row `height` do not.
            if let Some(w) = dim(attrs, "width", true) {
                css.push_str(&format!("width: {w};"));
            }
            if let Some(h) = dim(attrs, "height", false) {
                css.push_str(&format!("height: {h};"));
            }
            valign(attrs, &mut css);
        }
        "tr" => {
            if let Some(h) = dim(attrs, "height", false) {
                css.push_str(&format!("height: {h};"));
            }
            valign(attrs, &mut css);
        }
        "thead" | "tbody" | "tfoot" => valign(attrs, &mut css),
        "caption" if lower(attrs, "align").as_deref() == Some("bottom") => {
            css.push_str("caption-side: bottom;")
        }
        "ol" | "ul" | "li" => list_type(tag, attrs, &mut css),
        "embed" | "iframe" => embedded_align(attrs, &mut css),
        "img" | "object" => {
            embedded_align(attrs, &mut css);
            image_border(attrs, &mut css);
        }
        "input" if lower(attrs, "type").as_deref() == Some("image") => {
            embedded_align(attrs, &mut css);
            image_border(attrs, &mut css);
        }
        _ => {}
    }
    css
}

pub(super) fn rule(
    tag: &str,
    attrs: &[(String, String)],
    path: &[Ancestor],
    vcx: ValueCx<'_>,
) -> Option<Rule> {
    let css = hints(tag, attrs, path, vcx);
    if css.is_empty() {
        return None;
    }
    super::presentational_hints::hint(&css)
}
