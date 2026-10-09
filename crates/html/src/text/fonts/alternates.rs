//! Family-specific alternate names, CSS Fonts 4 §6.8–6.9.
//! Resolution follows inheritance and precedes font-feature-settings (§7.2).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

type Key = (String, String, String);
type Definition = (Vec<u32>, Vec<u32>);
/// Inherited requests retain their document's definitions, so parsing an iframe
/// cannot change the feature names of an already parsed outer document.
#[derive(Clone, Debug)]
pub struct Alternates {
    requests: Vec<(String, Vec<String>)>,
    values: Arc<HashMap<Key, Definition>>,
}

fn snapshot(requests: Vec<(String, Vec<String>)>) -> Alternates {
    Alternates {
        requests,
        values: VALUES.with(|v| Arc::new(v.borrow().clone())),
    }
}

pub(crate) fn normal() -> Alternates {
    snapshot(Vec::new())
}

thread_local! {
    static VALUES: RefCell<HashMap<Key, Definition>> = RefCell::new(HashMap::new());
}

pub(crate) fn reset() {
    VALUES.with(|v| v.borrow_mut().clear());
}

fn ident(s: &str) -> bool {
    let start = |c: char| c.is_ascii_alphabetic() || c == '_' || c as u32 >= 0x80;
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (start(first) || (first == '-' && chars.clone().next().is_some_and(|c| start(c) || c == '-')))
        && chars.all(|c| start(c) || c.is_ascii_digit() || c == '-')
}

fn kind(s: &str) -> bool {
    matches!(
        s,
        "stylistic" | "styleset" | "character-variant" | "swash" | "ornaments" | "annotation"
    )
}

pub(crate) fn register(families: &str, body: &str, layer: Vec<u32>) {
    let families: Vec<_> = families.split(',').map(str::trim).collect();
    if families.iter().any(|f| {
        !crate::computed::family_name_ok(f)
            || (!f.starts_with(['\'', '"']) && crate::computed::is_generic(&f.to_ascii_lowercase()))
    }) {
        return;
    }
    let families: Vec<_> = families
        .iter()
        .map(|f| {
            let name = crate::css::unescape(f.trim_matches(['\'', '"']));
            let name = if f.starts_with(['\'', '"']) {
                name
            } else {
                name.split_whitespace().collect::<Vec<_>>().join(" ")
            };
            name.to_ascii_lowercase()
        })
        .collect();
    let mut rest = body;
    while let Some((piece, tail)) = crate::css::next_piece(rest) {
        rest = tail;
        let crate::css::Piece::Block { head, body } = piece else {
            continue;
        };
        let group = head.trim().to_ascii_lowercase();
        let Some(group) = group.strip_prefix('@').filter(|s| kind(s)) else {
            continue;
        };
        // parse_decls lowercases declaration names; these aliases are case-sensitive.
        for decl in body.split(';') {
            let Some((name, value)) = decl.split_once(':') else {
                continue;
            };
            let name = crate::css::unescape(name.trim());
            let numbers: Option<Vec<u32>> =
                value.split_whitespace().map(|n| n.parse().ok()).collect();
            let Some(numbers) = numbers.filter(|n| !n.is_empty()) else {
                continue;
            };
            let valid = match group {
                "styleset" => numbers.iter().all(|n| *n <= 20),
                "character-variant" => numbers.len() <= 2 && numbers[0] <= 99,
                _ => numbers.len() == 1,
            };
            if !ident(&name) || !valid {
                continue;
            }
            VALUES.with(|v| {
                let mut v = v.borrow_mut();
                for family in &families {
                    let key = (family.clone(), group.into(), name.clone());
                    if v.get(&key).is_none_or(|old| old.0 <= layer) {
                        v.insert(key, (layer.clone(), numbers.clone()));
                    }
                }
            });
        }
    }
}

pub(crate) fn parse(value: &str, shorthand: bool) -> Option<Alternates> {
    if value.eq_ignore_ascii_case("normal") || (shorthand && value == "none") {
        return Some(normal());
    }
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(rest.len());
        let group = rest[..end].to_ascii_lowercase();
        rest = &rest[end..];
        if group == "historical-forms" {
            if out.iter().any(|(old, _)| old == &group) {
                return None;
            }
            out.push((group, Vec::new()));
        } else if rest.starts_with('(') && kind(&group) {
            let close = rest.find(')')?;
            let names: Vec<_> = rest[1..close]
                .split(',')
                .map(|n| crate::css::unescape(n.trim()))
                .collect();
            if names.iter().any(|n| !ident(n))
                || (!matches!(group.as_str(), "styleset" | "character-variant") && names.len() != 1)
                || out.iter().any(|(old, _)| old == &group)
            {
                return None;
            }
            out.push((group, names));
            rest = &rest[close + 1..];
        } else if !shorthand || group.is_empty() {
            return None;
        }
        rest = rest.trim_start();
    }
    Some(snapshot(out))
}

pub(crate) fn resolve(family: &str, alternates: Option<&Alternates>) -> Vec<(String, u32)> {
    let Some(alternates) = alternates else {
        return Vec::new();
    };
    let family = family.to_ascii_lowercase();
    let mut out = Vec::new();
    {
        let v = &alternates.values;
        for (group, names) in &alternates.requests {
            if group == "historical-forms" {
                out.push(("hist".into(), 1));
            }
            for name in names {
                let Some((_, numbers)) = v.get(&(family.clone(), group.clone(), name.clone()))
                else {
                    continue;
                };
                match group.as_str() {
                    "styleset" => {
                        for n in numbers.iter().filter(|n| **n != 0) {
                            out.push((format!("ss{n:02}"), 1));
                        }
                    }
                    "character-variant" if numbers[0] != 0 => {
                        out.push((
                            format!("cv{:02}", numbers[0]),
                            numbers.get(1).copied().unwrap_or(1),
                        ));
                    }
                    "stylistic" => out.push(("salt".into(), numbers[0])),
                    "swash" => {
                        out.push(("swsh".into(), numbers[0]));
                        out.push(("cswh".into(), numbers[0]));
                    }
                    "ornaments" => out.push(("ornm".into(), numbers[0])),
                    "annotation" => out.push(("nalt".into(), numbers[0])),
                    _ => {}
                }
            }
        }
    }
    out
}
