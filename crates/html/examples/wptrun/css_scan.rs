//! Top-level statement scan of a style sheet (css-syntax-3 §5.4.1 "consume a list of rules").
//!
//! The runner inlines `@import`ed sheets into `<style>`; to do that like a browser it must know
//! where each top-level statement starts and ends (comments, strings, `url(…)` and nested blocks
//! do not end it) and which constructs are still open at the sheet's end (EOF closes them, §5.4.1).

/// One top-level statement: `start..end` in the sheet text.
pub(super) struct Stmt {
    pub start: usize,
    pub end: usize,
    /// Lower-cased at-keyword name (`import`, `media`, …); `None` for a qualified rule.
    pub at: Option<String>,
    /// The statement ended with a `{…}` block (rather than `;` or EOF).
    pub block: bool,
}

/// Statements of a sheet plus the text that closes whatever EOF left open.
pub(super) struct Scan {
    pub stmts: Vec<Stmt>,
    /// Appending it makes the sheet self-contained: an unterminated comment, string, `url(`,
    /// bracket or trailing statement can no longer swallow text placed after the sheet.
    pub closer: String,
}

pub(super) fn scan(css: &str) -> Scan {
    let b = css.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut stmts = vec![];
    let mut closer = String::new();
    loop {
        // Whitespace, comments and CDO/CDC between top-level rules are skipped (§5.4.1).
        loop {
            if i < n && b[i].is_ascii_whitespace() {
                i += 1;
            } else if css[i..].starts_with("<!--") {
                i += 4;
            } else if css[i..].starts_with("-->") || css[i..].starts_with("]]>") {
                i += 3;
            } else if css[i..].starts_with("<![CDATA[") {
                // XHTML: the XML parser strips the section markers before CSS sees them.
                i += 9;
            } else if css[i..].starts_with("/*") {
                match css[i + 2..].find("*/") {
                    Some(e) => i += 2 + e + 2,
                    None => {
                        closer.push_str("*/");
                        i = n;
                    }
                }
            } else {
                break;
            }
        }
        if i >= n {
            break;
        }
        let start = i;
        let at = (b[i] == b'@').then(|| at_name(&css[i + 1..]));
        let mut stack: Vec<u8> = vec![];
        let mut block = false;
        let mut done = false;
        while i < n {
            let c = b[i];
            match c {
                b'/' if b.get(i + 1) == Some(&b'*') => match css[i + 2..].find("*/") {
                    Some(e) => i += 2 + e + 2,
                    None => {
                        closer.push_str("*/");
                        i = n;
                    }
                },
                b'"' | b'\'' => i = skip_string(b, i, &mut closer),
                b'\\' => i = (i + 2).min(n),
                b'(' if is_url_head(css, i) => i = skip_url(b, i + 1, &mut closer),
                b'(' | b'[' | b'{' => {
                    stack.push(match c {
                        b'(' => b')',
                        b'[' => b']',
                        _ => b'}',
                    });
                    i += 1;
                }
                b')' | b']' | b'}' => {
                    if stack.last() == Some(&c) {
                        stack.pop();
                        if c == b'}' && stack.is_empty() {
                            block = true;
                            done = true;
                        }
                    }
                    i += 1;
                }
                b';' if stack.is_empty() && at.is_some() => {
                    i += 1;
                    done = true;
                }
                _ => i += 1,
            }
            if done {
                break;
            }
        }
        if !done {
            // EOF inside the statement: close its brackets innermost first; a statement with no
            // block of its own still needs an end so it cannot absorb what follows.
            let opened_block = stack.first() == Some(&b'}');
            for c in stack.iter().rev() {
                closer.push(*c as char);
            }
            if opened_block {
                block = true;
            } else if at.is_some() {
                closer.push(';');
            } else {
                closer.push_str("{}");
            }
        }
        stmts.push(Stmt {
            start,
            end: i.min(n),
            at,
            block,
        });
        if !done {
            break;
        }
    }
    Scan { stmts, closer }
}

/// Lower-cased name of the at-keyword whose text follows `@`.
fn at_name(rest: &str) -> String {
    rest.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || !c.is_ascii())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Skip a string token from its opening quote; a newline ends it as a bad string (§4.3.5).
fn skip_string(b: &[u8], from: usize, closer: &mut String) -> usize {
    let q = b[from];
    let mut i = from + 1;
    while i < b.len() {
        match b[i] {
            c if c == q => return i + 1,
            b'\\' => i += 2,
            b'\n' => return i,
            _ => i += 1,
        }
    }
    closer.push(q as char);
    b.len()
}

/// `(` right after the name `url` (any case) opens a url token unless a quote follows (§4.3.4).
fn is_url_head(css: &str, paren: usize) -> bool {
    paren >= 3
        && css.is_char_boundary(paren - 3)
        && css[paren - 3..paren].eq_ignore_ascii_case("url")
        && !css[paren + 1..].trim_start().starts_with(['"', '\''])
}

/// Skip an unquoted url token's body up to and including `)`.
fn skip_url(b: &[u8], from: usize, closer: &mut String) -> usize {
    let mut i = from;
    while i < b.len() {
        match b[i] {
            b')' => return i + 1,
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
    closer.push(')');
    b.len()
}

/// Split at top-level commas (outside brackets and strings).
pub(super) fn split_commas(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut parts = vec![];
    let (mut depth, mut start, mut i) = (0i32, 0usize, 0usize);
    let mut sink = String::new();
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_string(b, i, &mut sink);
                continue;
            }
            b'\\' => i += 1,
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&text[start.min(text.len())..]);
    parts
}
