//! Слова значения сокращений сетки.

/// Слова значения сетки верхнего уровня: строки, группы имён `[…]` и
/// функции — целиком, `/` — отдельное слово.
pub(super) fn tokens(v: &str) -> Option<Vec<String>> {
    let (mut out, mut cur, mut depth, mut quote) = (Vec::new(), String::new(), 0i32, None);
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        }
    };
    for ch in v.chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if ch == q {
                quote = None;
                if depth == 0 {
                    flush(&mut cur, &mut out);
                }
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                if depth == 0 {
                    flush(&mut cur, &mut out);
                }
                quote = Some(ch);
                cur.push(ch);
            }
            '[' => {
                if depth == 0 {
                    flush(&mut cur, &mut out);
                }
                depth += 1;
                cur.push(ch);
            }
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ']' | ')' => {
                depth -= 1;
                cur.push(ch);
                if depth < 0 {
                    return None;
                }
                if depth == 0 && ch == ']' {
                    flush(&mut cur, &mut out);
                }
            }
            '/' if depth == 0 => {
                flush(&mut cur, &mut out);
                out.push("/".into());
            }
            c if c.is_whitespace() && depth == 0 => flush(&mut cur, &mut out),
            c => cur.push(c),
        }
    }
    if quote.is_some() || depth != 0 {
        return None;
    }
    flush(&mut cur, &mut out);
    Some(out)
}
