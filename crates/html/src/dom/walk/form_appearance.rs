//! The basic appearance UA sheet participates below author declarations.

use crate::style::computed::{Appearance, Computed};
use crate::style::css::{Rule, Selector, parse_decls};

pub(super) fn rule(tag: &str, attrs: &[(String, String)], style: &Computed) -> Option<Rule> {
    if style.appearance != Some(Appearance::Base) || !matches!(tag, "input" | "textarea") {
        return None;
    }
    if tag == "input"
        && attrs.iter().any(|(k, v)| {
            k == "type"
                && matches!(
                    v.to_ascii_lowercase().as_str(),
                    "button"
                        | "submit"
                        | "reset"
                        | "color"
                        | "file"
                        | "range"
                        | "checkbox"
                        | "radio"
                        | "hidden"
                )
        })
    {
        return None;
    }
    // CSS Forms 1, basic appearance stylesheet: inherit typography and
    // colours, transparent surface, currentColor outline. This runs in the
    // cascade, before inheritance and length normalisation, never in paint.
    Some(Rule {
        sel: Selector::parse(tag)?,
        decls: parse_decls(
            "color: inherit; font: inherit; box-sizing: border-box; \
             background-color: transparent; border: 1px solid currentColor; \
             border-radius: 0; padding: 0",
        ),
        origin: 0,
        order: usize::MAX,
        layer: vec![u32::MAX],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_defaults_stay_below_author_css() {
        let base = rule(
            "input",
            &[],
            &Computed {
                appearance: Some(Appearance::Base),
                ..Computed::default()
            },
        )
        .unwrap();
        let author = Rule {
            sel: Selector::parse("*").unwrap(),
            decls: parse_decls("color: red; padding: 7px"),
            origin: 1,
            order: 0,
            layer: vec![u32::MAX],
        };
        let mut rules = vec![&author, &base];
        let style = Computed::resolve_with_vars(&mut rules, &parse_decls(""), &parse_decls(""));
        assert_eq!(
            style.color,
            crate::style::values::value::Color::parse("red")
        );
        assert_eq!(
            style.padding.left,
            Some(crate::style::values::value::Len::Px(7.))
        );
        assert!(rule("input", &[], &Computed::default()).is_none());
    }
}
