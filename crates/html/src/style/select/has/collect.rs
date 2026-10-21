//! `:has()`: сбор аргументов из селектора до построения паспортов узлов —
//! рекурсивно через `:is()`/`:not()`/`:host()`/`:nth-*(... of S)` и
//! комбинаторы, включая аргументы, вычисляемые в другой области дерева.
//! Вложенный `:has()` делает селектор негодным.

use crate::style::css::{Selector, default_ns, nth_of_parts, split_selector_list};

/// Reject nested `:has()` and register every argument before building node passports.
pub(crate) fn collect_has_args(sel: &Selector, out: &mut Vec<String>) -> bool {
    for p in sel.pseudo.iter().chain(&sel.also) {
        let Some((name, arg)) = p.split_once('(') else {
            continue;
        };
        let Some(arg) = arg.strip_suffix(')') else {
            continue;
        };
        if name == "has" {
            if arg.to_ascii_lowercase().contains("has(") {
                return false;
            }
            if !out.iter().any(|a| a == arg) {
                out.push(arg.to_string());
            }
        } else {
            let list = match name {
                "is" | "where" | "matches" | "any" => default_ns::is_args(arg),
                "not" | "host" | "host-context" | "slotted" | "has-slotted" => {
                    split_selector_list(arg)
                        .into_iter()
                        .filter_map(Selector::parse)
                        .collect()
                }
                _ => nth_of_parts(arg).map_or_else(Vec::new, |(_, list)| list),
            };
            if !list.iter().all(|s| collect_has_args(s, out)) {
                return false;
            }
        }
    }
    sel.ancestor
        .as_ref()
        .is_none_or(|a| collect_has_args(&a.0, out))
        && sel
            .prev
            .as_ref()
            .is_none_or(|p| collect_has_args(&p.0, out))
}
