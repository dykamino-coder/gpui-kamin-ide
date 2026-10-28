//! Сбор аргументов :has() в прежнем порядке: псевдоклассы, предок, сосед.
//! Вложенный :has() — синтаксическая ошибка; отказ worklist — отдельный Result.

use crate::budget::admission::{Admission, Buffer, Error};
use crate::style::css::{Selector, TypeNamespace, default_ns, nth_of_parts, split_selector_list};

enum Task {
    Selector(Selector),
    Pseudo(String, TypeNamespace),
}

pub(crate) fn retain_valid_rules(
    admission: &Admission,
    rules: &mut Vec<crate::style::css::Rule>,
    out: &mut Vec<String>,
) -> Result<(), Error> {
    let mut failure = None;
    rules.retain(|rule| {
        if failure.is_some() {
            return false;
        }
        match collect_has_args(admission, &rule.sel, out) {
            Ok(valid) => valid,
            Err(error) => {
                failure = Some(error);
                false
            }
        }
    });
    failure.map_or(Ok(()), Err)
}

pub(crate) fn collect_has_args(
    admission: &Admission,
    sel: &Selector,
    out: &mut Vec<String>,
) -> Result<bool, Error> {
    let mut stack = Buffer::default();
    stack.push(Task::Selector(sel.clone()), admission)?;
    while let Some(task) = stack.pop() {
        admission.work(1)?;
        match task {
            Task::Selector(mut sel) => {
                if let Some(prev) = sel.prev.take() {
                    stack.push(Task::Selector(prev.0), admission)?;
                }
                if let Some(ancestor) = sel.ancestor.take() {
                    stack.push(Task::Selector(ancestor.0), admission)?;
                }
                for pseudo in sel.pseudo.into_iter().chain(sel.also).rev() {
                    stack.push(Task::Pseudo(pseudo, sel.argument_namespace), admission)?;
                }
            }
            Task::Pseudo(pseudo, namespace) => {
                let Some((name, arg)) = pseudo.split_once('(') else {
                    continue;
                };
                let Some(arg) = arg.strip_suffix(')') else {
                    continue;
                };
                if name == "has" {
                    if arg.to_ascii_lowercase().contains("has(") {
                        return Ok(false);
                    }
                    admission.work(out.len())?;
                    if !out.iter().any(|a| a == arg) {
                        out.push(arg.to_string());
                    }
                    continue;
                }
                let list = match name {
                    "is" | "where" | "matches" | "any" => default_ns::is_args(arg, namespace),
                    "not" | "host" | "host-context" | "slotted" | "has-slotted" => {
                        split_selector_list(arg)
                            .into_iter()
                            .filter_map(Selector::parse)
                            .collect()
                    }
                    _ => nth_of_parts(arg).map_or_else(Vec::new, |(_, list)| list),
                };
                for sel in list.into_iter().rev() {
                    stack.push(Task::Selector(sel), admission)?;
                }
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_then_ancestor_then_previous_and_nested_lists_keep_order() {
        let mut selector = Selector::parse(":has(.own)").unwrap();
        selector.ancestor = Some(Box::new((
            Selector::parse(":is(:has(.a),:has(.b))").unwrap(),
            false,
        )));
        selector.prev = Some(Box::new((Selector::parse(":has(.prev)").unwrap(), false)));
        let mut out = Vec::new();
        assert!(collect_has_args(&Admission::default(), &selector, &mut out).unwrap());
        assert_eq!(out, [".own", ".a", ".b", ".prev"]);
    }
}
