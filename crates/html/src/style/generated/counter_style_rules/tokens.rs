//! Лексер дескрипторов @counter-style: имена, экранирование, токены.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
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

pub(super) fn tokenize(v: &str) -> Vec<Tok> {
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
                && chars
                    .get(i + 1)
                    .is_some_and(|n| is_name_start(*n) || *n == '-')
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
                let mut m = Math {
                    s: text.as_bytes(),
                    i: 0,
                };
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
        if c.is_ascii_digit()
            || ((c == '-' || c == '+') && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
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
