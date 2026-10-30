//! Вычисление пользовательских свойств на их владельце до наследования
//! потомками: `compute` подставляет `var()` внутри `--*` с обнаружением циклов
//! (CSS Values 5 §substitution-context — негодны все участники), общими
//! CSS-словами и проверкой синтаксиса `@property`; `substitute` — общая
//! подстановка `var()` с запасным аргументом (CSS Variables §3), пробелы вокруг
//! подстановки берегут границы токенов. Весь DAG пользовательских свойств элемента
//! делит один бюджет работы; порядок обхода имён детерминирован, поэтому исчерпание
//! делает негодными одни и те же свойства при каждом вычислении.

use super::{Registered, VarMap};
use crate::budget::admission::Buffer;
use std::collections::{HashMap, HashSet};

pub fn compute(
    raw: &VarMap,
    parent: &VarMap,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
    work: &Work<'_>,
) -> VarMap {
    let mut resolver = Resolver {
        raw,
        parent,
        registered,
        accepts,
        stack: Buffer::default(),
        cyclic: HashSet::new(),
        cache: HashMap::new(),
    };
    let mut names: Vec<_> = raw.keys().filter(|name| name.starts_with("--")).collect();
    names.sort_unstable();
    names
        .into_iter()
        .filter_map(|name| {
            let value = resolver.value(name, work)?;
            Some((name.clone(), value))
        })
        .collect()
}

struct Resolver<'a, F> {
    raw: &'a VarMap,
    parent: &'a VarMap,
    registered: &'a HashMap<String, Registered>,
    accepts: F,
    stack: Buffer<String>,
    cyclic: HashSet<String>,
    cache: HashMap<String, Option<String>>,
}

impl<F: Fn(&str, &str) -> bool> Resolver<'_, F> {
    fn value(&mut self, name: &str, work: &Work<'_>) -> Option<String> {
        if let Some(value) = self.cache.get(name) {
            return value.clone();
        }
        let raw = self.raw.get(name)?.clone();
        self.run(name, &raw, work)
    }

    fn run(&mut self, name: &str, raw: &str, work: &Work<'_>) -> Option<String> {
        let value = self.run_frames(name, raw, work);
        // Отказ admission не оставляет active path для следующего корня DAG.
        self.stack = Buffer::default();
        value
    }

    fn run_frames(&mut self, name: &str, raw: &str, work: &Work<'_>) -> Option<String> {
        use substitution::{Expansion, Step};
        struct Frame {
            name: String,
            expansion: Result<Expansion, Unsubstituted>,
        }
        let mut frames = Buffer::default();
        work.admit(frames.push(
            Frame {
                name: name.into(),
                expansion: Expansion::start(raw, work),
            },
            work.admission(),
        ))
        .ok()?;
        work.admit(self.stack.push(name.into(), work.admission()))
            .ok()?;
        loop {
            let step = match &mut frames.last_mut().expect("active variable frame").expansion {
                Ok(expansion) => expansion.step(work),
                Err(error) => Err(*error),
            };
            if let Ok(Step::Need(dependency)) = step {
                let cached = self.cache.get(&dependency).cloned();
                let value = if let Some(value) = cached {
                    value
                } else {
                    if let Some(at) = self.stack.iter().position(|entry| entry == &dependency) {
                        // Негодны все участники текущего substitution-context; не весь visited DAG.
                        self.cyclic.extend(self.stack[at..].iter().cloned());
                    } else if let Some(raw) = self.raw.get(&dependency) {
                        work.admit(self.stack.push(dependency.clone(), work.admission()))
                            .ok()?;
                        work.admit(frames.push(
                            Frame {
                                name: dependency,
                                expansion: Expansion::start(raw, work),
                            },
                            work.admission(),
                        ))
                        .ok()?;
                        continue;
                    }
                    None
                };
                let frame = frames.last_mut().expect("requesting variable frame");
                if let Ok(expansion) = &mut frame.expansion
                    && let Err(error) = expansion.supply(value, work)
                {
                    frame.expansion = Err(error);
                }
                continue;
            }
            let frame = frames.pop().expect("completed variable frame");
            self.stack.pop();
            let value = match step {
                Ok(Step::Done(value)) => Some(value),
                _ => None,
            };
            let value = if frame.name.is_empty() {
                value
            } else {
                self.computed(&frame.name, value)
            };
            if frames.is_empty() {
                return value;
            }
            let parent = frames.last_mut().expect("variable parent");
            if let Ok(expansion) = &mut parent.expansion
                && let Err(error) = expansion.supply(value, work)
            {
                parent.expansion = Err(error);
            }
        }
    }

    fn computed(&mut self, name: &str, value: Option<String>) -> Option<String> {
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
mod work;
pub use substitution::has_var;
#[cfg(test)]
pub use substitution::substitute;
pub use work::{Unsubstituted, Work};

/// Обычное свойство использует тот же resolver, что custom properties: без потолка цепочки.
pub fn resolve(raw: &str, vars: &VarMap, work: &Work<'_>) -> Option<String> {
    Resolver {
        raw: vars,
        parent: &VarMap::new(),
        registered: &HashMap::new(),
        accepts: |_: &str, _: &str| true,
        stack: Buffer::default(),
        cyclic: HashSet::new(),
        cache: HashMap::new(),
    }
    .run("", raw, work)
}

#[cfg(test)]
#[path = "variable_values/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "variable_values/limits_tests.rs"]
mod limits_tests;

#[cfg(test)]
#[path = "variable_values/budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "variable_values/graph_tests.rs"]
mod graph_tests;
