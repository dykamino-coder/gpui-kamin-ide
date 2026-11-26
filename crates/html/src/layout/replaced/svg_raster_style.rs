//! Project the replaced box's paint inputs out of its isolated SVG content.

use crate::dom::Element;
use crate::style::css::{parse_decls, strip_comments};
use std::fmt::Write;

pub(super) fn content(e: &Element) -> Element {
    let mut content = e.clone();
    content.style.used_mut().reset_background.background = None;
    // CSS Color 4: opacity groups the decorations and content once, at the
    // outer CSS box. Presentation attributes without a computed CSS owner
    // remain part of the isolated SVG document.
    if e.style.used().reset_effects.opacity.is_some() {
        content.style.used_mut().reset_effects.opacity = None;
        let style = without_owned_opacity(e.attr("style").unwrap_or_default());
        content
            .attrs
            .retain(|(name, _)| name != "style" && name != "opacity");
        content.attrs.push(("style".into(), style));
    }
    content
}

fn without_owned_opacity(raw: &str) -> String {
    let mut out = String::new();
    // Parse before adding an override: an unfinished comment, string or block
    // at EOF must not swallow the new declaration. Keep custom token values.
    for decl in &parse_decls(&strip_comments(raw)) {
        if decl.name == "opacity" {
            continue;
        }
        for ch in decl.name.chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                out.push(ch);
            } else {
                write!(out, "\\{:x} ", ch as u32).unwrap();
            }
        }
        out.push(':');
        out.push_str(&decl.value);
        if decl.important {
            out.push_str("!important");
        }
        out.push(';');
    }
    // An embedded SVG stylesheet can also target the root. Inline importance
    // keeps that raster root neutral while the outer box consumes the cascade.
    out.push_str("opacity:1!important;");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    #[test]
    fn projection_keeps_svg_presentation_and_does_not_mutate_dom() {
        let mut e = Element::synthetic("svg", Computed::default(), Default::default(), vec![]);
        e.attrs.push(("opacity".into(), ".4".into()));
        assert_eq!(content(&e).attr("opacity"), Some(".4"));
        e.style.used_mut().reset_effects.opacity = Some(0.5);
        let projected = content(&e);
        assert_eq!(e.attr("opacity"), Some(".4"));
        assert_eq!(e.attr("style"), None);
        assert_eq!(projected.attr("opacity"), None);
        assert_eq!(
            parse_decls(projected.attr("style").unwrap()).last("opacity"),
            Some("1")
        );
        assert_eq!(e.style.used().reset_effects.opacity, Some(0.5));
        assert_eq!(projected.style.used().reset_effects.opacity, None);
    }

    #[test]
    fn css_tokens_cannot_escape_the_owned_effect_projection() {
        for raw in [
            "opacity:.5!important;--label:'a;b';fill:green",
            "op\\61 city:.5;--label:'a;b';fill:green/* unfinished",
            "opacity:.5;--label:'a;b';fill:green;--unfinished:'x",
        ] {
            let projected = without_owned_opacity(raw);
            let decls = parse_decls(&projected);
            assert_eq!(decls.last("opacity"), Some("1"), "{projected}");
            assert_eq!(decls.last("--label"), Some("'a;b'"));
            assert_eq!(decls.last("fill"), Some("green"));
            assert!(
                decls
                    .iter()
                    .filter(|d| d.name == "opacity")
                    .all(|d| d.important)
            );
        }
    }
}
