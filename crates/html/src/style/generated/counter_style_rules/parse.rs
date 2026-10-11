//! Разбор дескрипторов @counter-style (system, symbols, additive-symbols, range, pad) и регистрация правила.

use super::*;

/// `<symbol> = <string> | <image> | <custom-ident>` (картинки не
/// поддерживаются — такой символ делает значение недействительным).
fn symbol(t: &Tok) -> Option<String> {
    match t {
        Tok::Str(s) => Some(s.clone()),
        Tok::Ident(s) if !is_wide_keyword(s) => Some(s.clone()),
        _ => None,
    }
}

fn is_wide_keyword(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
    )
}

fn parse_system(t: &[Tok]) -> Option<System> {
    let Some(Tok::Ident(k)) = t.first() else {
        return None;
    };
    let k = k.to_ascii_lowercase();
    let sys = match (k.as_str(), &t[1..]) {
        ("cyclic", []) => System::Cyclic,
        ("numeric", []) => System::Numeric,
        ("alphabetic", []) => System::Alphabetic,
        ("symbolic", []) => System::Symbolic,
        ("additive", []) => System::Additive,
        ("fixed", []) => System::Fixed(1),
        ("fixed", [Tok::Int(n) | Tok::Calc(n)]) => System::Fixed(*n),
        ("extends", [Tok::Ident(name)])
            if !name.eq_ignore_ascii_case("none") && !is_wide_keyword(name) =>
        {
            System::Extends(normalize_name(name).into_owned())
        }
        _ => return None,
    };
    Some(sys)
}

fn parse_symbols(t: &[Tok]) -> Option<Vec<String>> {
    let v: Option<Vec<String>> = t.iter().map(symbol).collect();
    v.filter(|v| !v.is_empty())
}

fn parse_additive(t: &[Tok]) -> Option<Vec<(u64, String)>> {
    let mut out: Vec<(u64, String)> = vec![];
    for part in t.split(|x| *x == Tok::Comma) {
        let (w, s) = match part {
            [Tok::Int(w), s] | [s, Tok::Int(w)] => (*w, symbol(s)?),
            [Tok::Calc(w), s] | [s, Tok::Calc(w)] => ((*w).max(0), symbol(s)?),
            _ => return None,
        };
        if w < 0 {
            return None;
        }
        let w = w as u64;
        // Веса строго убывают (§additive-symbols).
        if out.last().is_some_and(|(prev, _)| *prev <= w) {
            return None;
        }
        out.push((w, s));
    }
    (!out.is_empty()).then_some(out)
}

fn parse_range(t: &[Tok]) -> Option<Option<Vec<(i64, i64)>>> {
    if let [Tok::Ident(k)] = t
        && k.eq_ignore_ascii_case("auto")
    {
        return Some(None);
    }
    let bound = |x: &Tok, low: bool| match x {
        Tok::Int(n) | Tok::Calc(n) => Some(*n),
        Tok::Ident(k) if k.eq_ignore_ascii_case("infinite") => {
            Some(if low { i64::MIN } else { i64::MAX })
        }
        _ => None,
    };
    let mut out = vec![];
    for part in t.split(|x| *x == Tok::Comma) {
        let [a, b] = part else { return None };
        let (lo, hi) = (bound(a, true)?, bound(b, false)?);
        if lo > hi {
            return None;
        }
        out.push((lo, hi));
    }
    (!out.is_empty()).then_some(Some(out))
}

fn parse_pad(t: &[Tok]) -> Option<(usize, String)> {
    match t {
        [Tok::Int(n), s] | [s, Tok::Int(n)] if *n >= 0 => Some((*n as usize, symbol(s)?)),
        [Tok::Calc(n), s] | [s, Tok::Calc(n)] => Some(((*n).max(0) as usize, symbol(s)?)),
        _ => None,
    }
}

/// Действительное значение дескриптора: при повторе берётся последнее
/// ДЕЙСТВИТЕЛЬНОЕ (недействительное объявление отбрасывается разбором).
fn last_valid<T>(values: &[String], parse: impl Fn(&[Tok]) -> Option<T>) -> Option<T> {
    values.iter().rev().find_map(|v| parse(&tokenize(v)))
}

/// Правило `@counter-style <имя> { … }`. `descs` — пары «дескриптор —
/// значения в порядке записи».
pub fn register(name: &str, descs: &[(String, Vec<String>)]) {
    let toks = tokenize(name.trim());
    let [Tok::Ident(name)] = toks.as_slice() else {
        return;
    };
    // §counter-style-name: `none` и непереопределяемые имена правило не
    // задают; предопределённые имена — строчными.
    let name = normalize_name(name).into_owned();
    if name.eq_ignore_ascii_case("none")
        || is_wide_keyword(&name)
        || matches!(
            name.as_str(),
            "decimal" | "disc" | "square" | "circle" | "disclosure-open" | "disclosure-closed"
        )
    {
        return;
    }
    let get = |k: &str| {
        descs
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    };
    let raw = Raw {
        system: last_valid(get("system"), parse_system),
        symbols: last_valid(get("symbols"), parse_symbols),
        additive: last_valid(get("additive-symbols"), parse_additive),
        negative: last_valid(get("negative"), |t| match t {
            [a] => Some((symbol(a)?, String::new())),
            [a, b] => Some((symbol(a)?, symbol(b)?)),
            _ => None,
        }),
        prefix: last_valid(get("prefix"), |t| match t {
            [a] => symbol(a),
            _ => None,
        }),
        suffix: last_valid(get("suffix"), |t| match t {
            [a] => symbol(a),
            _ => None,
        }),
        range: last_valid(get("range"), parse_range),
        pad: last_valid(get("pad"), parse_pad),
        fallback: last_valid(get("fallback"), |t| match t {
            [Tok::Ident(n)] if !n.eq_ignore_ascii_case("none") && !is_wide_keyword(n) => {
                Some(normalize_name(n).into_owned())
            }
            _ => None,
        }),
    };
    // Правило без нужных системе символов недействительно (§symbols,
    // §additive-symbols, §extends-system).
    let nsym = raw.symbols.as_ref().map_or(0, Vec::len);
    let valid = match raw.system.as_ref().unwrap_or(&System::Symbolic) {
        System::Cyclic | System::Fixed(_) | System::Symbolic => nsym >= 1,
        System::Alphabetic | System::Numeric => nsym >= 2,
        System::Additive => raw.additive.is_some(),
        System::Extends(_) => raw.symbols.is_none() && raw.additive.is_none(),
    };
    if !valid {
        return;
    }
    RULES
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(name, raw);
}
