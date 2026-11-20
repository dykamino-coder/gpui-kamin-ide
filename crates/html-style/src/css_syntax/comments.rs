//! Удаление комментариев вне строк и URL по CSS Syntax §4.3.2.

use super::{at_url, skip_string, skip_url};

pub fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut at = 0usize;
    while at < css.len() {
        let ch = css[at..].chars().next().unwrap_or('\0');
        // Кавычки и экранирование сильнее комментария: `content: "/*"` — это
        // текст, а не начало комментария до конца таблицы.
        match ch {
            '\\' => {
                let next = css[at + ch.len_utf8()..].chars().next();
                out.push(ch);
                if let Some(n) = next {
                    out.push(n);
                    at += ch.len_utf8() + n.len_utf8();
                } else {
                    at += ch.len_utf8();
                }
                continue;
            }
            '"' | '\'' => {
                let body = at + ch.len_utf8();
                let end = body + skip_string(&css[body..], ch);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ if at_url(&css[at..]) => {
                let end = at + skip_url(&css[at..]);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ => {}
        }
        if css[at..].starts_with("/*") {
            // A comment produces no token (CSS Syntax §4.3.2): `div/**/.a` is
            // the compound `div.a`. Parsing goes on over text, so a space
            // stands in only where the neighbours would otherwise merge into
            // one token (the pairs §9 separates in serialization), e.g.
            // `hsla(120/**/75% ...)` keeps two numbers.
            let next = css[at + 2..]
                .find("*/")
                .and_then(|end| css[at + 2 + end + 2..].chars().next());
            if let (Some(prev), Some(next)) = (out.chars().next_back(), next)
                && would_merge(prev, next)
            {
                out.push(' ');
            }
            match css[at + 2..].find("*/") {
                Some(end) => at += 2 + end + 2,
                // Незакрытый комментарий тянется до конца файла (§4.3.2).
                None => return out,
            }
            continue;
        }
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

/// Whether the token ending in `prev` and the one starting with `next` would
/// read as one token when written together (CSS Syntax §9, the pairs that need
/// a comment between them; ASCII whitespace never merges).
fn would_merge(prev: char, next: char) -> bool {
    let name =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '\\') || !c.is_ascii();
    // A percentage before a number is two tokens, but the value parsers read
    // text split at spaces: keep them apart there too.
    ((name(prev) || matches!(prev, '#' | '@')) && (name(next) || matches!(next, '(' | '%')))
        || (prev == '%' && name(next))
        || (matches!(prev, '.' | '+') && next.is_ascii_digit())
        || (prev.is_ascii_digit() && next == '.')
        || (prev == '/' && next == '*')
        || (prev == '<' && next == '!')
}

#[cfg(test)]
mod tests {
    use super::strip_comments;

    #[test]
    fn a_comment_separates_only_tokens_that_would_merge() {
        assert_eq!(strip_comments("div/**/.a{}"), "div.a{}");
        assert_eq!(strip_comments("a/* x */>b{}"), "a>b{}");
        assert_eq!(
            strip_comments("hsla(120/**/75%/**/50%)"),
            "hsla(120 75% 50%)"
        );
        assert_eq!(strip_comments("1/**/.5 a/**/b x/**/(y)"), "1 .5 a b x (y)");
        assert_eq!(strip_comments("p /* x */ i"), "p  i");
        assert_eq!(strip_comments("/* a */p{}"), "p{}");
    }
}
