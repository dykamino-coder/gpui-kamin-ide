//! Compute custom-property substitutions on their owner before descendants inherit them.

use super::{Decls, Registered};
use std::collections::{HashMap, HashSet};

pub(crate) fn compute(
    raw: &Decls,
    parent: &Decls,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
) -> Decls {
    let mut resolver = Resolver {
        raw,
        parent,
        registered,
        accepts,
        stack: Vec::new(),
        cyclic: HashSet::new(),
        cache: HashMap::new(),
    };
    raw.keys()
        .filter(|name| name.starts_with("--"))
        .filter_map(|name| resolver.value(name).map(|value| (name.clone(), value)))
        .collect()
}

struct Resolver<'a, F> {
    raw: &'a Decls,
    parent: &'a Decls,
    registered: &'a HashMap<String, Registered>,
    accepts: F,
    stack: Vec<String>,
    cyclic: HashSet<String>,
    cache: HashMap<String, Option<String>>,
}

impl<F: Fn(&str, &str) -> bool> Resolver<'_, F> {
    fn value(&mut self, name: &str) -> Option<String> {
        if let Some(value) = self.cache.get(name) {
            return value.clone();
        }
        if let Some(at) = self.stack.iter().position(|entry| entry == name) {
            // CSS Values 5 §substitution-context: invalidate every participant,
            // including a property whose var() fallback would mask the cycle.
            self.cyclic.extend(self.stack[at..].iter().cloned());
            return None;
        }
        let raw = self.raw.get(name)?.clone();
        self.stack.push(name.to_string());
        let value = substitute(&raw, &mut |dependency| self.value(dependency));
        self.stack.pop();
        let reg = self.registered.get(name);
        let initial = reg.and_then(|r| r.initial.as_deref());
        let inherits = reg.is_none_or(|r| r.inherits);
        let value = if self.cyclic.contains(name) {
            None
        } else {
            value
        };
        let wide = value.as_deref().is_some_and(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "initial" | "inherit" | "unset"
            )
        });
        let value = value.and_then(|v| {
            super::custom_properties::keyword(
                &v,
                self.parent.get(name).map(String::as_str),
                initial,
                inherits,
            )
        });
        let value = if let Some(reg) = reg.filter(|r| !wide && r.syntax.trim() != "*") {
            value
                .filter(|v| (self.accepts)(&reg.syntax, v))
                .or_else(|| {
                    if reg.inherits {
                        self.parent.get(name).cloned()
                    } else {
                        None
                    }
                    .or_else(|| reg.initial.clone())
                })
        } else {
            value
        };
        self.cache.insert(name.to_string(), value.clone());
        value
    }
}

struct Call {
    start: usize,
    end: usize,
    name: String,
    fallback: Option<String>,
}

pub(crate) fn has_var(raw: &str) -> bool {
    !calls(raw).is_empty()
}

/// Only selected fallback arguments are substituted (CSS Variables §3).
/// Whitespace preserves token boundaries: substituting `20` before `px`
/// must not manufacture a dimension token that was absent from the input.
pub(crate) fn substitute(
    raw: &str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Option<String> {
    let mut result = String::new();
    let mut from = 0;
    for call in calls(raw) {
        result.push_str(&raw[from..call.start]);
        let replacement = match lookup(&call.name) {
            Some(value) => value,
            None => substitute(call.fallback.as_deref()?, lookup)?,
        };
        result.push(' ');
        result.push_str(&replacement);
        result.push(' ');
        from = call.end;
    }
    result.push_str(&raw[from..]);
    Some(result)
}

fn calls(raw: &str) -> Vec<Call> {
    use super::variable_tokens::{block_end, escape_end, string_end};
    let mut found = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let ch = raw[at..].chars().next().unwrap();
        if matches!(ch, '\'' | '"') {
            at = string_end(raw, at, ch).unwrap_or(raw.len());
            continue;
        }
        if raw[at..].starts_with("/*") {
            at += raw[at + 2..].find("*/").map_or(raw.len() - at, |n| n + 4);
            continue;
        }
        let start = at;
        if ch == '\\' || ch == '-' || ch == '_' || ch.is_alphanumeric() || ch >= '\u{80}' {
            while at < raw.len() {
                let c = raw[at..].chars().next().unwrap();
                if c == '\\' {
                    at = escape_end(raw, at);
                } else if c.is_alphanumeric() || matches!(c, '-' | '_') || c >= '\u{80}' {
                    at += c.len_utf8();
                } else {
                    break;
                }
            }
            if raw[start..at].eq_ignore_ascii_case("var") && raw[at..].starts_with('(') {
                let Some(end) = block_end(raw, at, '(') else {
                    break;
                };
                let inner = super::strip_comments(&raw[at + 1..end]);
                let args = super::split_top_level(&inner, ',');
                found.push(Call {
                    start,
                    end: (end + 1).min(raw.len()),
                    // parse_decls has already decoded identifier escapes.
                    name: args[0].trim().to_string(),
                    fallback: (args.len() > 1).then(|| args[1..].join(",")),
                });
                at = (end + 1).min(raw.len());
            }
        }
        if at == start {
            at += ch.len_utf8();
        }
    }
    found
}

#[cfg(test)]
#[path = "variable_values/tests.rs"]
mod tests;
