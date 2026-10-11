//! Cascade custom-property metadata before exposing values to var() substitution.

use super::{Decls, Registered, Rule};
use std::collections::HashMap;

pub(crate) fn cascade(
    matched: &[&Rule],
    inline: &Decls,
    parent: &Decls,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
) -> Decls {
    let mut selected = Decls::new();
    for important in [false, true] {
        let mut ordered = matched.to_vec();
        if important {
            ordered.sort_by(|a, b| {
                (
                    std::cmp::Reverse(a.origin),
                    std::cmp::Reverse(&a.layer),
                    a.sel.specificity(),
                    a.order,
                )
                    .cmp(&(
                        std::cmp::Reverse(b.origin),
                        std::cmp::Reverse(&b.layer),
                        b.sel.specificity(),
                        b.order,
                    ))
            });
        } else {
            ordered.sort_by(|a, b| {
                (a.origin, &a.layer, a.sel.specificity(), a.order).cmp(&(
                    b.origin,
                    &b.layer,
                    b.sel.specificity(),
                    b.order,
                ))
            });
        }
        let mut inline_done = false;
        for rule in ordered {
            // Important UA declarations outrank author inline declarations
            // (CSS Cascade §6.2); normal inline declarations still come last.
            if important && rule.origin == 0 && !inline_done {
                select(&mut selected, inline, important);
                inline_done = true;
            }
            select(&mut selected, &rule.decls, important);
        }
        if !inline_done {
            select(&mut selected, inline, important);
        }
    }
    let mut result = parent.clone();
    for (name, value) in &selected {
        let reg = registered.get(name);
        let initial = reg.and_then(|r| r.initial.as_deref());
        let inherits = reg.is_none_or(|r| r.inherits);
        // CSS Variables §2: CSS-wide words are cascade instructions, not
        // the token stream later substituted into an ordinary declaration.
        let value = keyword(
            value,
            parent.get(name).map(String::as_str),
            initial,
            inherits,
        );
        match value {
            Some(value) => {
                result.insert(name.clone(), value);
            }
            None => {
                result.remove(name);
            }
        }
    }
    for (name, reg) in registered {
        let fallback = if reg.inherits {
            parent.get(name).cloned()
        } else {
            None
        }
        .or_else(|| reg.initial.clone());
        let value = if let Some(raw) = selected.get(name) {
            // A CSS-wide initial must stay invalid when the registered
            // universal syntax has no initial value; it cannot inherit.
            if matches!(
                raw.to_ascii_lowercase().as_str(),
                "initial" | "inherit" | "unset"
            ) {
                result.get(name).cloned()
            } else {
                result
                    .get(name)
                    .cloned()
                    .filter(|v| accepts(&reg.syntax, v))
                    .or(fallback)
            }
        } else {
            fallback
        };
        match value {
            Some(value) => {
                result.insert(name.clone(), value);
            }
            None => {
                result.remove(name);
            }
        }
    }
    result
}

fn select(selected: &mut Decls, decls: &Decls, important: bool) {
    for (name, raw) in decls {
        if !name.starts_with("--") {
            continue;
        }
        let bang = super::top_level_bang(raw);
        let metadata = decls.get(&format!("{}{name}", super::CUSTOM_IMPORTANT));
        let priority = metadata.map_or(bang.is_some(), |v| v == "1");
        if priority == important {
            let value = if metadata.is_some() {
                raw.as_str()
            } else {
                bang.map_or(raw.as_str(), |at| raw[..at].trim_end())
            };
            selected.insert(name.clone(), value.to_string());
        }
    }
}

pub(super) fn keyword(
    value: &str,
    parent: Option<&str>,
    initial: Option<&str>,
    inherits: bool,
) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "initial" => initial,
        "inherit" => parent.or(initial),
        "unset" if inherits => parent.or(initial),
        "unset" => initial,
        _ => Some(value),
    }
    .map(str::to_string)
}

pub(super) fn store(decls: &mut Decls, name: String, value: String, important: bool) {
    let priority = format!("{}{name}", super::CUSTOM_IMPORTANT);
    if decls.get(&priority).is_some_and(|v| v == "1") && !important {
        return;
    }
    decls.insert(priority, if important { "1" } else { "0" }.to_string());
    decls.insert(name, value);
}

#[cfg(test)]
mod tests {
    use super::super::{Decls, Rule, Selector, parse_decls};
    use super::{cascade, keyword};
    use std::collections::HashMap;

    #[test]
    fn repeated_declarations_keep_priority_and_literal_bangs() {
        let decls = parse_decls("--a:green!important;--a:red");
        let values = cascade(&[], &decls, &Decls::new(), &HashMap::new(), |_, _| true);
        assert_eq!(values["--a"], "green");
        let decls = parse_decls(r"--a:green\!important");
        let values = cascade(&[], &decls, &Decls::new(), &HashMap::new(), |_, _| true);
        assert_eq!(values["--a"], "green!important");
    }

    #[test]
    fn wide_keywords_use_custom_property_inheritance() {
        assert_eq!(keyword("initial", Some("red"), None, true), None);
        assert_eq!(
            keyword("inherit", Some("green"), None, true).as_deref(),
            Some("green")
        );
        assert_eq!(keyword("unset", None, None, true), None);
        assert_eq!(
            keyword("unset", Some("red"), Some("green"), false).as_deref(),
            Some("green")
        );
        assert_eq!(
            keyword("inherit", Some("red"), Some("green"), false).as_deref(),
            Some("red")
        );
        assert_eq!(keyword("", Some("red"), None, true).as_deref(), Some(""));
    }

    #[test]
    fn important_metadata_is_removed_after_cascade_selection() {
        let rule = |value: &str, order| Rule {
            sel: Selector::parse("p").unwrap(),
            decls: parse_decls(value),
            order,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let first = rule("--a: green !important", 0);
        let last = rule("--a: red", 1);
        let values = cascade(
            &[&first, &last],
            &Decls::new(),
            &Decls::new(),
            &HashMap::new(),
            |_, _| true,
        );
        assert_eq!(values.get("--a").map(String::as_str), Some("green"));
    }

    #[test]
    fn inherited_and_initial_variables_are_distinct_from_empty_values() {
        let parent = parse_decls("--a: green; --b: red");
        let inline = parse_decls("--a: inherit; --b: initial; --empty:");
        let values = cascade(&[], &inline, &parent, &HashMap::new(), |_, _| true);
        assert_eq!(values.get("--a").map(String::as_str), Some("green"));
        assert!(!values.contains_key("--b"));
        assert_eq!(values.get("--empty").map(String::as_str), Some(""));
    }
}
