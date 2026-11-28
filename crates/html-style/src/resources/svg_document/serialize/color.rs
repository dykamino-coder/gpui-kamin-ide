//! Preserve computed CSS color when an inline SVG becomes an isolated document.

use crate::dom::Element;
use crate::style::values::value::Color;

/// CSS Color 4 section 6.4: currentColor uses this element's computed color.
pub(super) fn write(e: &Element, inherited: Color, out: &mut String) -> Color {
    let color = e.style.used().current_color();
    if color != inherited {
        out.push_str(&format!(
            " color=\"rgba({}, {}, {}, {})\"",
            color.r * 255.0,
            color.g * 255.0,
            color.b * 255.0,
            color.a
        ));
    }
    color
}

/// The computed value already includes inline declarations and author priority.
pub(super) fn without_inline_color(value: &str) -> String {
    crate::style::css::split_top_level(value, ';')
        .into_iter()
        .filter(|decl| !color_declaration(decl))
        .collect::<Vec<_>>()
        .join(";")
}

pub(super) fn property_name(value: &str) -> Option<String> {
    let parts = crate::style::css::split_top_level(value, ':');
    (parts.len() > 1).then(|| {
        crate::style::css::unescape(&crate::style::css::strip_comments(parts[0]))
            .trim()
            .to_ascii_lowercase()
    })
}

fn color_declaration(value: &str) -> bool {
    property_name(value).is_some_and(|name| name == "color")
}

/// Do not cascade color a second time in the isolated rasterizer. Keep other
/// SVG properties, including properties that only the rasterizer understands.
pub(super) fn stylesheet(value: &str) -> String {
    use crate::style::css::{Piece, next_piece, split_top_level};
    enum Work<'a> {
        List(&'a str),
        Part(&'a str),
        Text(&'a str),
    }
    let source = crate::style::css::strip_comments(value);
    let mut work = vec![Work::List(&source)];
    let mut out = String::new();
    while let Some(item) = work.pop() {
        match item {
            Work::Text(text) => out.push_str(text),
            Work::List(text) => {
                let parts = split_top_level(text, ';');
                for (index, part) in parts.into_iter().enumerate().rev() {
                    work.push(Work::Part(part));
                    if index > 0 {
                        work.push(Work::Text(";"));
                    }
                }
            }
            Work::Part(text) => match next_piece(text) {
                Some((piece, rest)) => {
                    work.push(Work::Part(rest));
                    match piece {
                        Piece::Block { head, body } => {
                            out.push_str(head);
                            out.push('{');
                            // Custom property values are opaque component values.
                            if property_name(head).is_some_and(|name| name.starts_with("--")) {
                                out.push_str(body);
                                out.push('}');
                            } else {
                                work.push(Work::Text("}"));
                                work.push(Work::List(body));
                            }
                        }
                        Piece::Statement { head } => {
                            out.push_str(head);
                            out.push(';');
                        }
                    }
                }
                None if !color_declaration(text) => out.push_str(text),
                None => {}
            },
        }
    }
    out
}
