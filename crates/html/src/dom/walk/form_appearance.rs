//! The basic appearance UA sheet participates below author declarations.

use crate::style::computed::{Appearance, Computed};
use crate::style::css::{Rule, Selector, parse_decls};

pub(super) fn rule(tag: &str, attrs: &[(String, String)], style: &Computed) -> Option<Rule> {
    if style.appearance != Some(Appearance::Base) || !matches!(tag, "input" | "textarea" | "button")
    {
        return None;
    }
    let button = tag == "button"
        || (tag == "input"
            && attrs.iter().any(|(k, v)| {
                k == "type"
                    && matches!(
                        v.to_ascii_lowercase().as_str(),
                        "button" | "submit" | "reset"
                    )
            }));
    if !button
        && tag == "input"
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
    let selector = if button && tag == "input" {
        "input[type]"
    } else {
        tag
    };
    let declarations = if button {
        "color: inherit; font: inherit; box-sizing: border-box; background-color: transparent; \
         border: 1px solid currentColor; padding-block: .25em; padding-inline: .5em; \
         border-radius: .5em; display: inline-flex; align-items: center; justify-content: center; \
         gap: .5em; min-width: max(24px, 1lh); min-height: max(24px, 1lh); \
         text-transform: initial; text-align: start; text-indent: initial; user-select: none"
    } else {
        "color: inherit; font: inherit; box-sizing: border-box; \
             background-color: transparent; border: 1px solid currentColor; \
             border-radius: 0; padding: 0"
    };
    Some(Rule {
        sel: Selector::parse(selector)?,
        decls: parse_decls(declarations),
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
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
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
        let style =
            Computed::resolve_with_vars(&mut rules, &parse_decls(""), &parse_decls(""), vcx);
        assert_eq!(
            style.color,
            crate::style::values::value::Color::parse("red", vcx)
        );
        assert_eq!(
            style.padding.left,
            Some(crate::style::values::value::Len::Px(7.))
        );
        assert!(rule("input", &[], &Computed::default()).is_none());
    }
}
