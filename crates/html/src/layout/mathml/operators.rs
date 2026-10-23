//! Словарь операторов MathML Core и явные свойства операторов.
use crate::style::computed::BoxStyle;
use crate::style::values::value::px_strict;
use crate::{
    dom::{Element, Node},
    layout::options::RenderOpts,
};
#[path = "operator_data0.rs"]
mod data0;
#[path = "operator_data1.rs"]
mod data1;
#[path = "operator_data2.rs"]
mod data2;
#[path = "operator_data3.rs"]
mod data3;
#[path = "operator_data4.rs"]
mod data4;
#[path = "operator_data5.rs"]
mod data5;
type Entry = (&'static str, u8, u8, u8, u8);
#[derive(Clone, Copy)]
pub(super) struct Operator {
    pub left: f32,
    pub right: f32,
    pub flags: u8,
}
pub(super) fn text(e: &Element) -> String {
    e.children
        .iter()
        .filter_map(|n| {
            if let Node::Text(s) = n {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}
pub(super) fn core(e: &Element, parent: &BoxStyle, opts: &RenderOpts) -> Option<Operator> {
    let vcx = opts.cx.vcx();
    let style = crate::style::cascade::materialize::box_style(parent, &e.style, vcx);
    if e.tag == "mo" {
        return Some(operator(e, &style, opts));
    }
    let mut children = e.children.iter().filter_map(|n| {
        if let Node::Element(e) = n {
            Some(e)
        } else {
            None
        }
    });
    let first = children.next()?;
    if matches!(
        e.tag.as_str(),
        "msub" | "msup" | "msubsup" | "mmultiscripts" | "munder" | "mover" | "munderover"
    ) || (matches!(
        e.tag.as_str(),
        "math" | "mrow" | "mstyle" | "mpadded" | "mphantom" | "semantics"
    ) && children.next().is_none())
    {
        core(first, &style, opts)
    } else {
        None
    }
}
fn lookup(text: &str, form: u8) -> Option<Entry> {
    [
        data0::ENTRIES,
        data1::ENTRIES,
        data2::ENTRIES,
        data3::ENTRIES,
        data4::ENTRIES,
        data5::ENTRIES,
    ]
    .iter()
    .find_map(|entries| {
        entries
            .binary_search_by(|entry| (entry.0, entry.1).cmp(&(text, form)))
            .ok()
            .map(|i| entries[i])
    })
}
pub(super) fn operator(e: &Element, style: &BoxStyle, opts: &RenderOpts) -> Operator {
    let text = text(e);
    let form = match e.attr("form") {
        Some("prefix") => 1,
        Some("postfix") => 2,
        Some("infix") => 0,
        _ => style.math_operator_form.unwrap_or(0),
    };
    let entry = lookup(&text, form)
        .or_else(|| lookup(&text, 0))
        .or_else(|| lookup(&text, 2))
        .or_else(|| lookup(&text, 1));
    let (_, _, l, r, mut flags) = entry.unwrap_or(("", 0, 5, 5, 0));
    for (name, flag) in [
        ("stretchy", 1),
        ("symmetric", 2),
        ("largeop", 4),
        ("movablelimits", 8),
    ] {
        match e.attr(name) {
            Some("true") => flags |= flag,
            Some("false") => flags &= !flag,
            _ => {}
        }
    }
    let size = px_strict(style.font_size).unwrap_or(opts.base_size());
    let spacing = |name, v| {
        e.attr(name)
            .and_then(|v| super::parameters::length_px(v, style, opts))
            .unwrap_or(size * f32::from(v) / 18.0)
            .max(0.0)
    };
    let invisible = text.chars().count() == 1
        && text
            .chars()
            .next()
            .is_some_and(|c| matches!(c, '\u{2061}'..='\u{2064}'));
    let suppressed = style.math_level > 0 || invisible;
    Operator {
        left: if suppressed {
            0.0
        } else {
            spacing("lspace", l)
        },
        right: if suppressed {
            0.0
        } else {
            spacing("rspace", r)
        },
        flags,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normative_dictionary_covers_single_and_multi_character_operators() {
        assert_eq!(lookup("*", 0).map(|e| (e.2, e.3)), Some((3, 3)));
        assert!(lookup("\u{223f}", 0).is_none());
        assert_eq!(lookup("!=", 0).map(|e| (e.2, e.3)), Some((5, 5)));
        assert!(lookup("(", 1).is_some_and(|e| e.4 & 3 == 3));
    }
}
