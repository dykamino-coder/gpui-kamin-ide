//! Каскад пользовательских свойств (`--*`) до подстановки `var()`: выбор
//! выигравшего объявления по происхождению, слою, специфичности, порядку и
//! `!important` (важное UA старше встроенного авторского, CSS Cascade §6.2),
//! обработка общих CSS-слов как команд каскада (CSS Variables §2), а для
//! зарегистрированных `@property` — проверка синтаксиса и запасное
//! начальное/унаследованное значение.

use super::{Registered, VarMap};
use crate::style::cascade::map::{CascadeMap, Pick};
use std::collections::HashMap;

/// Пользовательские свойства элемента по его карте каскада.
pub(crate) fn cascade_map(
    map: &CascadeMap<'_>,
    parent: &VarMap,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
) -> VarMap {
    // Тот же ключ каскада и те же откаты, что у обычных свойств
    // (css-cascade-5 §6.2, §7.2–7.3; CSS Variables §2): важное агента
    // старше встроенного авторского, `revert`/`revert-layer` откатывают по
    // истории кандидатов, откат без цели — `unset`.
    let mut selected = VarMap::new();
    for name in map.names().iter().filter(|k| k.starts_with("--")) {
        match map.picks(name).next() {
            Some(Pick::Value(c, _)) => {
                selected.insert(name.to_string(), c.decl.value.clone());
            }
            Some(Pick::Unset(_)) => {
                selected.insert(name.to_string(), "unset".into());
            }
            None => {}
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

#[cfg(test)]
mod tests {
    use super::super::{DeclBlock, Rule, Selector, VarMap, parse_decls};
    use super::{cascade_map, keyword};
    use crate::style::cascade::map::CascadeMap;
    use crate::style::css::Registered;
    use std::collections::HashMap;

    fn cascade(
        matched: &[&Rule],
        inline: &DeclBlock,
        parent: &VarMap,
        registered: &HashMap<String, Registered>,
        accepts: impl Fn(&str, &str) -> bool,
    ) -> VarMap {
        cascade_map(
            &CascadeMap::collect(matched, inline),
            parent,
            registered,
            accepts,
        )
    }

    #[test]
    fn repeated_declarations_keep_priority_and_literal_bangs() {
        let decls = parse_decls("--a:green!important;--a:red");
        let values = cascade(&[], &decls, &VarMap::new(), &HashMap::new(), |_, _| true);
        assert_eq!(values["--a"], "green");
        let decls = parse_decls(r"--a:green\!important");
        let values = cascade(&[], &decls, &VarMap::new(), &HashMap::new(), |_, _| true);
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
            origin: super::super::Origin::Author,
            layer: vec![u32::MAX],
        };
        let first = rule("--a: green !important", 0);
        let last = rule("--a: red", 1);
        let values = cascade(
            &[&first, &last],
            &DeclBlock::new(),
            &VarMap::new(),
            &HashMap::new(),
            |_, _| true,
        );
        assert_eq!(values.get("--a").map(String::as_str), Some("green"));
    }

    #[test]
    fn inherited_and_initial_variables_are_distinct_from_empty_values() {
        let parent = VarMap::from([("--a".into(), "green".into()), ("--b".into(), "red".into())]);
        let inline = parse_decls("--a: inherit; --b: initial; --empty:");
        let values = cascade(&[], &inline, &parent, &HashMap::new(), |_, _| true);
        assert_eq!(values.get("--a").map(String::as_str), Some("green"));
        assert!(!values.contains_key("--b"));
        assert_eq!(values.get("--empty").map(String::as_str), Some(""));
    }
}
