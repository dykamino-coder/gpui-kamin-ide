//! Правила `@counter-style` документа (css-counter-styles-3 §3).
//!
//! Разбор стилевых листов кладёт сюда дескрипторы каждого действительного
//! правила (`register`), разбор следующего документа реестр чистит
//! (`reset`). Представление значения строится по алгоритму спеки
//! §counter-style-generate: диапазон → резерв, начальное представление по
//! системе, дополнение `pad`, знак `negative`. Предопределённые стили
//! описаны в `counter_style::builtin`; без правил документа путь
//! предопределённых стилей не меняется вовсе.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::counter_style::{builtin, builtin_initial, builtin_repr, normalize_name};

#[derive(Clone, Debug, PartialEq)]
enum System {
    Cyclic,
    Numeric,
    Alphabetic,
    Symbolic,
    Additive,
    Fixed(i64),
    Extends(String),
}

/// Дескрипторы одного правила, как они заданы (незаданное — `None`).
#[derive(Clone, Debug, Default)]
struct Raw {
    system: Option<System>,
    symbols: Option<Vec<String>>,
    additive: Option<Vec<(u64, String)>>,
    negative: Option<(String, String)>,
    prefix: Option<String>,
    suffix: Option<String>,
    /// `Some(None)` — `auto`.
    range: Option<Option<Vec<(i64, i64)>>>,
    pad: Option<(usize, String)>,
    fallback: Option<String>,
}

static RULES: Mutex<Option<HashMap<String, Raw>>> = Mutex::new(None);

pub fn reset() {
    RULES.lock().unwrap().take();
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Str(String),
    Ident(String),
    Int(i64),
    /// Целое из `calc()` (css-values-4 §10.9: вне допустимого диапазона
    /// значение не отбрасывается, а прижимается — это решает дескриптор).
    Calc(i64),
    Comma,
    Other,
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

fn is_name(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-'
}

/// Экранирование CSS (css-syntax-3 §4.3.7): до шести шестнадцатеричных цифр
/// и один пробельный символ после них, иначе — сам следующий знак.
fn escape(chars: &[char], i: &mut usize) -> char {
    // chars[*i] == '\\'
    *i += 1;
    let mut hex = String::new();
    while *i < chars.len() && hex.len() < 6 && chars[*i].is_ascii_hexdigit() {
        hex.push(chars[*i]);
        *i += 1;
    }
    if !hex.is_empty() {
        if *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
        let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
        return match char::from_u32(code) {
            Some(c) if code != 0 => c,
            _ => '\u{FFFD}',
        };
    }
    let c = chars.get(*i).copied().unwrap_or('\u{FFFD}');
    *i += 1;
    c
}

fn valid_escape(chars: &[char], i: usize) -> bool {
    chars.get(i) == Some(&'\\') && chars.get(i + 1).is_some_and(|c| *c != '\n')
}

fn tokenize(v: &str) -> Vec<Tok> {
    let chars: Vec<char> = v.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == ',' {
            out.push(Tok::Comma);
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != c {
                if chars[i] == '\\' {
                    if chars.get(i + 1) == Some(&'\n') {
                        i += 2;
                        continue;
                    }
                    s.push(escape(&chars, &mut i));
                } else {
                    s.push(chars[i]);
                    i += 1;
                }
            }
            i += 1;
            out.push(Tok::Str(s));
            continue;
        }
        let starts_ident = is_name_start(c)
            || valid_escape(&chars, i)
            || (c == '-'
                && chars.get(i + 1).is_some_and(|n| is_name_start(*n) || *n == '-')
                || (c == '-' && valid_escape(&chars, i + 1)));
        if starts_ident {
            // Математическая функция — до парной скобки.
            let word: String = chars[i..].iter().take_while(|c| is_name(**c)).collect();
            if chars.get(i + word.chars().count()) == Some(&'(') && is_math_fn(&word) {
                let start = i;
                let mut depth = 0;
                while i < chars.len() {
                    match chars[i] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let mut m = Math { s: text.as_bytes(), i: 0 };
                let v = m.func().filter(|v| !v.2).filter(|_| m.done());
                out.push(v.map_or(Tok::Other, |v| Tok::Calc(v.0.round() as i64)));
                continue;
            }
            let mut s = String::new();
            while i < chars.len() {
                if chars[i] == '\\' && valid_escape(&chars, i) {
                    s.push(escape(&chars, &mut i));
                } else if is_name(chars[i]) {
                    s.push(chars[i]);
                    i += 1;
                } else {
                    break;
                }
            }
            out.push(Tok::Ident(s));
            continue;
        }
        if c.is_ascii_digit() || ((c == '-' || c == '+') && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            // Дробное число или размерность целым не являются.
            if i < chars.len() && (chars[i] == '.' || is_name(chars[i]) || chars[i] == '%') {
                while i < chars.len() && !chars[i].is_whitespace() && chars[i] != ',' {
                    i += 1;
                }
                out.push(Tok::Other);
                continue;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(text.parse().map_or(Tok::Other, Tok::Int));
            continue;
        }
        out.push(Tok::Other);
        i += 1;
    }
    out
}

fn is_math_fn(w: &str) -> bool {
    matches!(
        w.to_ascii_lowercase().as_str(),
        "calc" | "sign" | "abs" | "min" | "max" | "clamp"
    )
}

/// Значение внутри `calc()`: (число, px, длина ли).
type MathVal = (f64, f64, bool);

/// Вычислитель `calc()` для целых дескрипторов. У дескрипторов правила нет
/// элемента, и `em`/`rem` берутся от начального кегля 16px (css-values-4
/// §6.1: где элемента нет, относительные единицы — от начальных значений).
struct Math<'a> {
    s: &'a [u8],
    i: usize,
}

impl Math<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn done(&mut self) -> bool {
        self.ws();
        self.i == self.s.len()
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Option<MathVal> {
        let mut a = self.term()?;
        loop {
            self.ws();
            let Some(&op) = self.s.get(self.i) else { return Some(a) };
            if op != b'+' && op != b'-' {
                return Some(a);
            }
            // Сложению нужны пробелы вокруг знака (§10.1).
            if !self.s.get(self.i + 1).is_some_and(u8::is_ascii_whitespace) {
                return None;
            }
            self.i += 1;
            let b = self.term()?;
            if a.2 != b.2 {
                return None;
            }
            let k = if op == b'+' { 1.0 } else { -1.0 };
            a = (a.0 + k * b.0, a.1 + k * b.1, a.2);
        }
    }
    fn term(&mut self) -> Option<MathVal> {
        let mut a = self.unit()?;
        loop {
            if self.eat(b'*') {
                let b = self.unit()?;
                a = match (a.2, b.2) {
                    (false, false) => (a.0 * b.0, 0.0, false),
                    (true, false) => (0.0, a.1 * b.0, true),
                    (false, true) => (0.0, b.1 * a.0, true),
                    _ => return None,
                };
            } else if self.eat(b'/') {
                let b = self.unit()?;
                if b.2 || b.0 == 0.0 {
                    return None;
                }
                a = (a.0 / b.0, a.1 / b.0, a.2);
            } else {
                return Some(a);
            }
        }
    }
    fn unit(&mut self) -> Option<MathVal> {
        self.ws();
        if self.eat(b'(') {
            let v = self.expr()?;
            return self.eat(b')').then_some(v);
        }
        if self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            return self.func();
        }
        let start = self.i;
        if matches!(self.s.get(self.i), Some(b'+' | b'-')) {
            self.i += 1;
        }
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_digit() || *c == b'.') {
            self.i += 1;
        }
        let n: f64 = std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()?;
        let us = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            self.i += 1;
        }
        let unit = std::str::from_utf8(&self.s[us..self.i]).ok()?.to_ascii_lowercase();
        let px = match unit.as_str() {
            "" => return Some((n, 0.0, false)),
            "px" => 1.0,
            "em" | "rem" => 16.0,
            "in" => 96.0,
            "cm" => 96.0 / 2.54,
            "mm" => 96.0 / 25.4,
            "pt" => 96.0 / 72.0,
            "pc" => 16.0,
            _ => return None,
        };
        Some((0.0, n * px, true))
    }
    /// Математическая функция по имени (css-values-4 §10).
    fn func(&mut self) -> Option<MathVal> {
        self.ws();
        let start = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            self.i += 1;
        }
        let name = std::str::from_utf8(&self.s[start..self.i]).ok()?.to_ascii_lowercase();
        if !self.eat(b'(') {
            return None;
        }
        let mut args = vec![self.expr()?];
        while self.eat(b',') {
            args.push(self.expr()?);
        }
        if !self.eat(b')') {
            return None;
        }
        let val = |a: &MathVal| if a.2 { a.1 } else { a.0 };
        let mk = |x: f64, len: bool| if len { (0.0, x, true) } else { (x, 0.0, false) };
        let same = args.iter().all(|a| a.2 == args[0].2);
        match (name.as_str(), args.as_slice()) {
            ("calc", [a]) => Some(*a),
            ("sign", [a]) => {
                let x = val(a);
                Some((if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }, 0.0, false))
            }
            ("abs", [a]) => Some(mk(val(a).abs(), a.2)),
            ("min", _) if same => {
                Some(mk(args.iter().map(val).fold(f64::INFINITY, f64::min), args[0].2))
            }
            ("max", _) if same => {
                Some(mk(args.iter().map(val).fold(f64::NEG_INFINITY, f64::max), args[0].2))
            }
            ("clamp", [lo, v, hi]) if same => {
                Some(mk(val(v).min(val(hi)).max(val(lo)), lo.2))
            }
            _ => None,
        }
    }
}

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
    let Some(Tok::Ident(k)) = t.first() else { return None };
    let k = k.to_ascii_lowercase();
    let sys = match (k.as_str(), &t[1..]) {
        ("cyclic", []) => System::Cyclic,
        ("numeric", []) => System::Numeric,
        ("alphabetic", []) => System::Alphabetic,
        ("symbolic", []) => System::Symbolic,
        ("additive", []) => System::Additive,
        ("fixed", []) => System::Fixed(1),
        ("fixed", [Tok::Int(n) | Tok::Calc(n)]) => System::Fixed(*n),
        ("extends", [Tok::Ident(name)]) if !name.eq_ignore_ascii_case("none") && !is_wide_keyword(name) => {
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
    let [Tok::Ident(name)] = toks.as_slice() else { return };
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
    let get = |k: &str| descs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_slice()).unwrap_or(&[]);
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
    RULES.lock().unwrap().get_or_insert_with(HashMap::new).insert(name, raw);
}

#[derive(Clone, Debug)]
enum Algo {
    Own(System, Vec<String>, Vec<(u64, String)>),
    Builtin(String),
}

#[derive(Clone, Debug)]
struct Resolved {
    algo: Algo,
    negative: (String, String),
    prefix: String,
    suffix: String,
    range: Option<Vec<(i64, i64)>>,
    pad: Option<(usize, String)>,
    fallback: String,
}

fn builtin_resolved(name: &str) -> Resolved {
    let b = builtin(name).or_else(|| builtin("decimal")).expect("decimal");
    let name = if builtin(name).is_some() { name } else { "decimal" };
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

fn resolve(rules: &HashMap<String, Raw>, name: &str, depth: usize) -> Option<Resolved> {
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

fn uses_negative(algo: &Algo) -> bool {
    match algo {
        Algo::Own(sys, ..) => matches!(
            sys,
            System::Numeric | System::Alphabetic | System::Symbolic | System::Additive
        ),
        Algo::Builtin(name) => builtin(name).is_some_and(|b| b.uses_negative),
    }
}

fn in_range(r: &Resolved, v: i64) -> bool {
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
        return if depth < 16 { builtin_repr(v, name) } else { v.to_string() };
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
fn anonymous(name: &str) -> Option<Raw> {
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
    let need = if matches!(system, System::Alphabetic | System::Numeric) { 2 } else { 1 };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn with(css: &[(&str, &[(&str, &str)])], f: impl FnOnce()) {
        reset();
        for (name, descs) in css {
            let d: Vec<(String, Vec<String>)> =
                descs.iter().map(|(k, v)| (k.to_string(), vec![v.to_string()])).collect();
            register(name, &d);
        }
        f();
        reset();
    }

    #[test]
    fn systems_follow_spec() {
        with(
            &[
                ("a", &[("system", "cyclic"), ("symbols", "\\2020  \\2021")]),
                ("b", &[("system", "extends upper-roman"), ("range", "infinite 5"), ("pad", "3 '*'")]),
                ("c", &[("system", "additive"), ("additive-symbols", "3 \"a\", 2 \"b\"")]),
                ("d", &[("system", "numeric"), ("symbols", "'0' '1' '2'")]),
            ],
            || {
                assert_eq!(custom_repr(-2, "a").unwrap(), "\u{2020}");
                assert_eq!(custom_repr(-1, "b").unwrap(), "-*I");
                assert_eq!(custom_repr(0, "b").unwrap(), "0");
                assert_eq!(custom_repr(6, "b").unwrap(), "6");
                assert_eq!(custom_repr(1, "c").unwrap(), "1");
                assert_eq!(custom_repr(5, "c").unwrap(), "ab");
                assert_eq!(custom_repr(-4, "d").unwrap(), "-11");
            },
        );
    }
}
