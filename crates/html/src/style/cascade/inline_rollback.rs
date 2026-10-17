//! Cascade rollback for element-attached declarations, before applying values.

use super::revert::{is_important, strip_important};
use crate::style::css::{DECL_SEP, Decls, Rule};

/// Inline styles have their own layer-comparison rank (CSS Cascade 5 §6.4).
/// `revert-layer` removes that rank; `revert` also removes author sheets.
pub(super) fn prepare(matched: &[&Rule], inline: &Decls) -> Option<(Vec<Rule>, Decls)> {
    let rollbacks: Vec<_> = inline
        .iter()
        // The all shorthand is handled by pass::all_reset.
        .filter(|(key, _)| key.as_str() != "all")
        .flat_map(|(key, value)| {
            [true, false].into_iter().filter_map(move |important| {
                let last = value
                    .split(DECL_SEP)
                    .rfind(|v| is_important(v) == important)?;
                let keyword = strip_important(last).trim();
                matches!(keyword, "revert" | "revert-layer").then_some((
                    key.clone(),
                    important,
                    keyword == "revert",
                ))
            })
        })
        .collect();
    if rollbacks.is_empty() {
        return None;
    }
    let mut rules: Vec<Rule> = matched.iter().map(|r| (*r).clone()).collect();
    let mut inline = inline.clone();
    for (key, important, origin) in rollbacks {
        remove(&mut inline, &key, important);
        if origin {
            for rule in rules.iter_mut().filter(|r| r.origin == 1) {
                remove(&mut rule.decls, &key, important);
            }
        }
    }
    Some((rules, inline))
}

fn remove(decls: &mut Decls, key: &str, important: bool) {
    if let Some(value) = decls.get(key) {
        let kept: Vec<_> = value
            .split(DECL_SEP)
            .filter(|part| !important && is_important(part))
            .collect();
        if kept.is_empty() {
            decls.remove(key);
        } else {
            let value = kept.join(&DECL_SEP.to_string());
            decls.insert(key.to_string(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;
    use crate::style::{
        computed::Computed,
        css::{Selector, parse_decls},
    };

    #[test]
    fn inline_rollback_preserves_origin_and_element_attached_rank() {
        let ua = Rule {
            sel: Selector::parse("p").unwrap(),
            decls: parse_decls("width: 10px"),
            origin: 0,
            order: 0,
            layer: vec![u32::MAX],
        };
        let author = Rule {
            decls: parse_decls("width: 20px"),
            origin: 1,
            ..ua.clone()
        };
        for (inline, expected) in [
            ("width: revert", 10.0),
            ("width: revert-layer", 20.0),
            ("width: 30px; width: revert !important", 10.0),
        ] {
            let c = Computed::resolve(&mut vec![&ua, &author], &parse_decls(inline));
            assert_eq!(c.width, Some(Len::Px(expected)), "{inline}");
        }
        let author = Rule {
            decls: parse_decls("width: revert-layer"),
            ..author
        };
        let c = Computed::resolve(&mut vec![&ua, &author], &Decls::new());
        assert_eq!(c.width, Some(Len::Px(10.0)));
    }
}
