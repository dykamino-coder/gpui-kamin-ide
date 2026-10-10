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

fn builtin_resolved(name: &str) -> Resolved {
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

/// Входит ли имя в цикл `extends` (§extends-system: все стили цикла ведут
/// себя как `extends decimal`).
fn in_cycle(rules: &HashMap<String, Raw>, name: &str) -> bool {
    let mut cur = name.to_string();
    for _ in 0..=rules.len() {
        let Some(System::Extends(next)) = rules.get(&cur).and_then(|r| r.system.clone()) else {
            return false;
        };
        if next == name {
            return true;
        }
        cur = next;
    }
    false
}

pub(super) fn resolve(rules: &HashMap<String, Raw>, name: &str, depth: usize) -> Option<Resolved> {
    let anon;
    let raw = match rules.get(name) {
        Some(raw) => raw,
        None => {
            anon = anonymous(name)?;
            &anon
        }
    };
    let mut r = match raw.system.clone().unwrap_or(System::Symbolic) {
        System::Extends(target) => {
            if depth > 32 || in_cycle(rules, name) {
                builtin_resolved("decimal")
            } else if rules.contains_key(&target) {
                resolve(rules, &target, depth + 1).unwrap_or_else(|| builtin_resolved("decimal"))
            } else {
                // Незнакомое имя — `extends decimal`.
                builtin_resolved(&target)
            }
        }
        sys => Resolved {
            algo: Algo::Own(
                sys,
                raw.symbols.clone().unwrap_or_default(),
                raw.additive.clone().unwrap_or_default(),
            ),
            negative: ("-".to_string(), String::new()),
            prefix: String::new(),
            suffix: ". ".to_string(),
            range: None,
            pad: None,
            fallback: "decimal".to_string(),
        },
    };
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
    Some(r)
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
