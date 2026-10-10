//! Запись числа пользовательским стилем счётчика: начальные системы, графемы, маркеры.

use super::*;

/// Начальное представление по системе (§cyclic … §additive).
fn initial(algo: &Algo, v: i64) -> Option<String> {
    match algo {
        Algo::Builtin(name) => builtin_initial(name, u64::try_from(v).ok()?),
        Algo::Own(sys, syms, add) => {
            let n = syms.len() as i64;
            match sys {
                System::Cyclic => Some(syms[(v - 1).rem_euclid(n) as usize].clone()),
                System::Fixed(first) => {
                    let i = v.checked_sub(*first)?;
                    (0..n).contains(&i).then(|| syms[i as usize].clone())
                }
                System::Symbolic => {
                    if v < 1 {
                        return None;
                    }
                    let reps = ((v - 1) / n + 1) as usize;
                    if reps > 60 {
                        return None;
                    }
                    Some(syms[((v - 1) % n) as usize].repeat(reps))
                }
                System::Alphabetic => {
                    if v < 1 {
                        return None;
                    }
                    let mut v = v;
                    let mut out = vec![];
                    while v > 0 {
                        v -= 1;
                        out.push(syms[(v % n) as usize].as_str());
                        v /= n;
                    }
                    out.reverse();
                    Some(out.concat())
                }
                System::Numeric => {
                    let mut v = v;
                    if v == 0 {
                        return Some(syms[0].clone());
                    }
                    let mut out = vec![];
                    while v > 0 {
                        out.push(syms[(v % n) as usize].as_str());
                        v /= n;
                    }
                    out.reverse();
                    Some(out.concat())
                }
                System::Additive => {
                    let mut v = v as u64;
                    if v == 0 {
                        return add.iter().find(|(w, _)| *w == 0).map(|(_, s)| s.clone());
                    }
                    let mut out = String::new();
                    let mut count = 0;
                    for (w, s) in add {
                        if *w == 0 || v < *w {
                            continue;
                        }
                        let reps = v / w;
                        count += reps;
                        if count > 1000 {
                            return None;
                        }
                        out.push_str(&s.repeat(reps as usize));
                        v -= reps * w;
                        if v == 0 {
                            break;
                        }
                    }
                    (v == 0).then_some(out)
                }
                System::Extends(_) => None,
            }
        }
    }
}

fn graphemes(s: &str) -> usize {
    unicode_segmentation::UnicodeSegmentation::graphemes(s, true).count()
}

/// §counter-style-generate для стиля `name` (уже нормализованного).
fn generate(rules: &HashMap<String, Raw>, v: i64, name: &str, depth: usize) -> String {
    let Some(r) = (depth < 16).then(|| resolve(rules, name, 0)).flatten() else {
        let v = v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        return if depth < 16 {
            builtin_repr(v, name)
        } else {
            v.to_string()
        };
    };
    let fallback = |rules: &HashMap<String, Raw>| {
        let fb = r.fallback.clone();
        if depth >= 15 || fb == name {
            v.to_string()
        } else {
            generate(rules, v, &fb, depth + 1)
        }
    };
    if !in_range(&r, v) {
        return fallback(rules);
    }
    let neg = v < 0 && uses_negative(&r.algo);
    let Some(mut body) = initial(&r.algo, if neg { v.saturating_neg() } else { v }) else {
        return fallback(rules);
    };
    if let Some((width, sym)) = &r.pad {
        let mut len = graphemes(&body);
        if neg {
            len += graphemes(&r.negative.0) + graphemes(&r.negative.1);
        }
        if len < *width {
            body = format!("{}{body}", sym.repeat(width - len));
        }
    }
    if neg {
        body = format!("{}{body}{}", r.negative.0, r.negative.1);
    }
    body
}

/// Представление значения стилем, заданным правилом документа; `None` —
/// такого правила нет (решает предопределённый путь).
pub fn custom_repr(v: i64, name: &str) -> Option<String> {
    let guard = RULES.lock().unwrap();
    let empty = HashMap::new();
    let rules = guard.as_ref().unwrap_or(&empty);
    (rules.contains_key(name) || anonymous(name).is_some()).then(|| generate(rules, v, name, 0))
}

/// Анонимный стиль `symbols()` (css-counter-styles-3 §symbols-function):
/// система (по умолчанию `symbolic`, `additive` и `extends` запрещены),
/// символы — только строки; суффикс — пробел, прочее — по умолчанию.
pub(super) fn anonymous(name: &str) -> Option<Raw> {
    let head = name.get(..8)?;
    if !head.eq_ignore_ascii_case("symbols(") {
        return None;
    }
    let inner = name[8..].trim_end().strip_suffix(')')?;
    let toks = tokenize(inner);
    let (system, rest) = match toks.first()? {
        Tok::Ident(k) => {
            let sys = match k.to_ascii_lowercase().as_str() {
                "cyclic" => System::Cyclic,
                "numeric" => System::Numeric,
                "alphabetic" => System::Alphabetic,
                "symbolic" => System::Symbolic,
                "fixed" => System::Fixed(1),
                _ => return None,
            };
            (sys, &toks[1..])
        }
        _ => (System::Symbolic, &toks[..]),
    };
    let symbols: Vec<String> = rest
        .iter()
        .map(|t| match t {
            Tok::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let need = if matches!(system, System::Alphabetic | System::Numeric) {
        2
    } else {
        1
    };
    if symbols.len() < need {
        return None;
    }
    Some(Raw {
        system: Some(system),
        symbols: Some(symbols),
        suffix: Some(" ".to_string()),
        ..Raw::default()
    })
}

/// Действительна ли запись `symbols(…)` — для разбора `list-style-type`.
pub fn valid_symbols_fn(text: &str) -> bool {
    anonymous(text).is_some()
}

/// Строка маркера: `prefix`, представление, `suffix` (§counter-style-prefix).
/// Префикс и суффикс берутся у самого стиля, даже если значение ушло в
/// резервный.
pub fn custom_marker(v: i64, name: &str) -> Option<String> {
    let guard = RULES.lock().unwrap();
    let empty = HashMap::new();
    let rules = guard.as_ref().unwrap_or(&empty);
    let r = resolve(rules, name, 0)?;
    let body = generate(rules, v, name, 0);
    Some(format!("{}{body}{}", r.prefix, r.suffix))
}
