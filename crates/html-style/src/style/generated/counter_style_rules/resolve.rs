//! Разрешение правила @counter-style: алгоритм, цепочка extends и запасные стили, диапазон.

use super::*;

#[derive(Clone, Debug)]
pub(super) enum Algo {
    Own(System, Vec<String>, Vec<(u64, String)>),
    Builtin(String),
}

#[derive(Clone, Debug)]
pub(super) struct Resolved {
    pub(super) algo: Algo,
    pub(super) negative: (String, String),
    pub(super) prefix: String,
    pub(super) suffix: String,
    pub(super) range: Option<Vec<(i64, i64)>>,
    pub(super) pad: Option<(usize, String)>,
    pub(super) fallback: String,
}

pub(super) fn builtin_resolved(name: &str) -> Resolved {
    // decimal входит в is_predefined; builtin возвращает Some для каждого такого имени.
    let b = builtin(name)
        .or_else(|| builtin("decimal"))
        .expect("decimal");
    let name = if builtin(name).is_some() {
        name
    } else {
        "decimal"
    };
    Resolved {
        algo: Algo::Builtin(name.to_string()),
        negative: b.negative,
        prefix: String::new(),
        suffix: b.suffix,
        range: b.range.map(|r| vec![r]),
        pad: b.pad,
        fallback: b.fallback.to_string(),
    }
}

/// Цепочка extends вычисляется снизу вверх. Каждый участник цикла наследует
/// decimal непосредственно; входящий в цикл стиль наследует его ближайшего участника.
pub(super) fn resolve_counter_style(
    rules: &HashMap<String, Raw>,
    name: &str,
    admission: &crate::budget::admission::Admission,
) -> Result<Option<Resolved>, crate::budget::admission::Error> {
    use crate::budget::admission::Buffer;
    let anon = anonymous(name);
    let Some(first) = rules.get(name).or(anon.as_ref()) else {
        return Ok(None);
    };
    let mut path = Buffer::default();
    let mut seen = HashMap::new();
    let mut current = name;
    let mut raw = first;
    let mut cycle = None;
    let mut result = loop {
        admission.work(1)?;
        seen.insert(current.to_string(), path.len());
        path.push(raw, admission)?;
        match raw.system.as_ref().unwrap_or(&System::Symbolic) {
            System::Extends(target) => {
                if let Some(&at) = seen.get(target) {
                    cycle = Some(at);
                    break builtin_resolved("decimal");
                }
                if let Some(next) = rules.get(target) {
                    current = target;
                    raw = next;
                } else {
                    break builtin_resolved(target);
                }
            }
            sys => {
                break Resolved {
                    algo: Algo::Own(
                        sys.clone(),
                        raw.symbols.clone().unwrap_or_default(),
                        raw.additive.clone().unwrap_or_default(),
                    ),
                    negative: ("-".into(), String::new()),
                    prefix: String::new(),
                    suffix: ". ".into(),
                    range: None,
                    pad: None,
                    fallback: "decimal".into(),
                };
            }
        }
    };
    while let Some(raw) = path.pop() {
        if cycle.is_some_and(|at| path.len() >= at) {
            result = builtin_resolved("decimal");
        }
        overlay(&mut result, raw);
    }
    Ok(Some(result))
}

fn overlay(r: &mut Resolved, raw: &Raw) {
    if let Some(v) = &raw.negative {
        r.negative = v.clone();
    }
    if let Some(v) = &raw.prefix {
        r.prefix = v.clone();
    }
    if let Some(v) = &raw.suffix {
        r.suffix = v.clone();
    }
    if let Some(v) = &raw.range {
        r.range = v.clone();
    }
    if let Some(v) = &raw.pad {
        r.pad = Some(v.clone());
    }
    if let Some(v) = &raw.fallback {
        r.fallback = v.clone();
    }
}

pub(super) fn uses_negative(algo: &Algo) -> bool {
    match algo {
        Algo::Own(sys, ..) => matches!(
            sys,
            System::Numeric | System::Alphabetic | System::Symbolic | System::Additive
        ),
        Algo::Builtin(name) => builtin(name).is_some_and(|b| b.uses_negative),
    }
}

pub(super) fn in_range(r: &Resolved, v: i64) -> bool {
    match &r.range {
        Some(list) => list.iter().any(|(lo, hi)| *lo <= v && v <= *hi),
        // `auto` (§counter-style-range).
        None => match &r.algo {
            Algo::Own(System::Alphabetic | System::Symbolic, ..) => v >= 1,
            Algo::Own(System::Additive, ..) => v >= 0,
            _ => true,
        },
    }
}
