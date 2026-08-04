//! Лексер выражений when: токены и разбор строки.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Op(String),
    Ident(String),
    Str(String),
    Num(f64),
    Regex(String, String), // (pattern, flags) — компилится на parse
}
const TWO_CHAR: [&str; 6] = ["&&", "||", "==", "!=", ">=", "<="];
fn ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn ident_body(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-')
}
pub(crate) fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        // Двухсимвольные операторы РАНЬШЕ одиночных (иначе `!=` = `!` + `=`)
        if i + 1 < b.len() {
            let two: String = b[i..i + 2].iter().collect();
            if TWO_CHAR.contains(&two.as_str()) || two == "=~" {
                toks.push(Tok::Op(two));
                i += 2;
                continue;
            }
        }
        if matches!(c, '(' | ')' | '!' | '<' | '>') {
            toks.push(Tok::Op(c.to_string()));
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                j += 1;
            }
            if j >= b.len() {
                return Err("unterminated string".into());
            }
            toks.push(Tok::Str(b[i + 1..j].iter().collect()));
            i = j + 1;
            continue;
        }
        if c == '/' {
            let mut j = i + 1;
            while j < b.len() && b[j] != '/' {
                if b[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            if j >= b.len() {
                return Err("unterminated regex".into());
            }
            let pattern: String = b[i + 1..j].iter().collect();
            j += 1;
            let mut flags = String::new();
            while j < b.len() && ident_start(b[j]) {
                flags.push(b[j]);
                j += 1;
            }
            toks.push(Tok::Regex(pattern, flags));
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == '.') {
                j += 1;
            }
            let s: String = b[i..j].iter().collect();
            toks.push(Tok::Num(s.parse().map_err(|_| "bad number")?));
            i = j;
            continue;
        }
        if ident_start(c) {
            let mut j = i;
            while j < b.len() && ident_body(b[j]) {
                j += 1;
            }
            toks.push(Tok::Ident(b[i..j].iter().collect()));
            i = j;
            continue;
        }
        return Err(format!("unexpected '{c}' at {i}"));
    }
    Ok(toks)
}
