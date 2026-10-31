//! Подготовленные функциональные аргументы принадлежат AST, а не процессному кэшу.

use super::*;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    pub raw: String,
    pub nth_cacheable: bool,
    pub list: Vec<Selector>,
    pub nth: Option<String>,
    pub single: bool,
    pub specificity: (u32, u32, u32),
}

impl Selector {
    pub fn prepared(&self, raw: &str) -> Option<&Prepared> {
        self.arguments.iter().find(|p| p.raw == raw).map(Rc::as_ref)
    }

    pub(super) fn prepare_arguments(&mut self) {
        self.arguments = self
            .pseudo
            .iter()
            .chain(&self.also)
            .map(|raw| {
                let mut list = Vec::new();
                let mut nth = None;
                if let Some((name, arg)) = raw.split_once('(')
                    && let Some(arg) = arg.strip_suffix(')')
                {
                    match name {
                        "is" | "where" | "matches" | "any" => {
                            list = crate::style::css::default_ns::is_args(
                                arg,
                                self.argument_namespace,
                            );
                        }
                        "host" | "slotted" => list.extend(Selector::parse(arg)),
                        "not" => {
                            list = split_selector_list(arg)
                                .into_iter()
                                .filter_map(Selector::parse)
                                .collect()
                        }
                        "has" => {
                            list = split_selector_list(arg)
                                .into_iter()
                                .filter_map(|s| {
                                    Selector::parse(s.trim().trim_start_matches(['>', '+', '~']))
                                })
                                .collect()
                        }
                        "has-slotted" => {
                            list = split_selector_list(arg)
                                .into_iter()
                                .filter_map(Selector::parse)
                                .collect()
                        }
                        "nth-child" | "nth-last-child" => {
                            if let Some((anb, parsed)) = nth_of_parts(arg) {
                                nth = Some(anb);
                                list = parsed;
                            }
                        }
                        _ => {}
                    }
                }
                let single = raw
                    .strip_prefix("not(")
                    .and_then(|s| s.strip_suffix(")"))
                    .is_none_or(|arg| split_selector_list(arg).len() == 1)
                    && list.len() == 1;
                let maximum = list
                    .iter()
                    .map(Selector::specificity)
                    .max()
                    .unwrap_or_default();
                let name = raw.split_once('(').map_or(raw.as_str(), |(name, _)| name);
                let specificity = match name {
                    "where" => (0, 0, 0),
                    "is" | "matches" | "any" | "has" => maximum,
                    "slotted" => (maximum.0, maximum.1, maximum.2 + 1),
                    "nth-child" | "nth-last-child" if nth.is_some() => {
                        (maximum.0, maximum.1 + 1, maximum.2)
                    }
                    _ => (0, 1, 0),
                };
                let lower = raw.to_ascii_lowercase();
                let nth_cacheable = !["link", "visited", "target", "host", "slotted"]
                    .iter()
                    .any(|name| lower.contains(name));
                Rc::new(Prepared {
                    raw: raw.clone(),
                    nth_cacheable,
                    list,
                    nth,
                    single,
                    specificity,
                })
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_arguments_keep_case_namespace_and_nth_list() {
        let sel = Selector::parse(":is(.Upper, :unknown):nth-child(2n of .Keep)").unwrap();
        assert_eq!(
            sel.prepared("is(.Upper, :unknown)").unwrap().list[0].classes,
            ["Upper"]
        );
        let nth = sel.prepared("nth-child(2n of .keep)").unwrap();
        assert_eq!(nth.nth.as_deref(), Some("2n"));
        assert_eq!(nth.list.len(), 1);
    }
}
