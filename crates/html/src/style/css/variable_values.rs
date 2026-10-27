//! Вычисление пользовательских свойств на их владельце до наследования
//! потомками: `compute` подставляет `var()` внутри `--*` с обнаружением циклов
//! (CSS Values 5 §substitution-context — негодны все участники), общими
//! CSS-словами и проверкой синтаксиса `@property`; `substitute` — общая
//! подстановка `var()` с запасным аргументом (CSS Variables §3), пробелы вокруг
//! подстановки берегут границы токенов. Весь DAG пользовательских свойств элемента
//! делит один бюджет работы; порядок обхода имён детерминирован, поэтому исчерпание
//! делает негодными одни и те же свойства при каждом вычислении.

use super::{Registered, VarMap};
use std::collections::{HashMap, HashSet};

pub(crate) fn compute(
    raw: &VarMap,
    parent: &VarMap,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
    revision: &crate::budget::RevisionBudget,
) -> VarMap {
    let work = Work::new(revision);
    let mut resolver = Resolver {
        raw,
        parent,
        registered,
        accepts,
        stack: Vec::new(),
        cyclic: HashSet::new(),
        cache: HashMap::new(),
    };
    let mut names: Vec<_> = raw.keys().filter(|name| name.starts_with("--")).collect();
    names.sort_unstable();
    names
        .into_iter()
        .filter_map(|name| {
            let value = resolver.value(name, &work)?;
            Some((name.clone(), value))
        })
        .collect()
}

struct Resolver<'a, F> {
    raw: &'a VarMap,
    parent: &'a VarMap,
    registered: &'a HashMap<String, Registered>,
    accepts: F,
    stack: Vec<String>,
    cyclic: HashSet<String>,
    cache: HashMap<String, Option<String>>,
}

impl<F: Fn(&str, &str) -> bool> Resolver<'_, F> {
    fn value(&mut self, name: &str, work: &Work<'_>) -> Option<String> {
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
        let value = substitute(&raw, &mut |dependency| self.value(dependency, work), work).ok();
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

mod substitution;
pub(crate) use substitution::{SubstitutionLimit, Unsubstituted, Work, has_var, substitute};

#[cfg(test)]
#[path = "variable_values/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "variable_values/limits_tests.rs"]
mod limits_tests;

#[cfg(test)]
#[path = "variable_values/budget_tests.rs"]
mod budget_tests;
