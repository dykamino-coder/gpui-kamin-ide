//! Упоминание имени в тексте CSS: консервативное множество признаков листа.
//!
//! Blink перед перерасчётом стиля смотрит в `RuleFeatureSet`, упоминает ли
//! какой-нибудь селектор изменённый класс или атрибут. Здесь то же решается по
//! тексту листа без разбора: каждая последовательность символов
//! идентификатора (буквы, цифры, `-`, `_`, не-ASCII и экранирования CSS
//! Syntax §4.3.7) сравнивается с именем без учёта ASCII-регистра — в
//! селекторах, значениях, строках и комментариях. Имя атрибута в селекторе,
//! `attr()` или `@container style()` — такой идентификатор где угодно; класс
//! селектор называет только записью `.имя` (Selectors 4 §6.6; `[class~=…]`
//! требует имени атрибута `class`), поэтому класс ищется лишь после
//! неэкранированной точки. «Не упомянуто» означает «ни одно правило от имени
//! не зависит»; лишнее упоминание лишь отменяет короткий путь.

/// Упоминает ли `css` имя атрибута из `attrs` где угодно или класс из
/// `classes` после точки (имена — в нижнем ASCII-регистре).
pub(super) fn mentions(css: &str, attrs: &[String], classes: &[String]) -> bool {
    let mut word = String::new();
    let mut dotted = false;
    let mut chars = css.chars().peekable();
    loop {
        let c = chars.next();
        match c {
            Some(c) if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii() => {
                word.push(c.to_ascii_lowercase());
                continue;
            }
            Some('\\') => {
                if let Some(c) = escaped(&mut chars) {
                    word.push(c.to_ascii_lowercase());
                    continue;
                }
            }
            _ => {}
        }
        if !word.is_empty() && (attrs.contains(&word) || dotted && classes.contains(&word)) {
            return true;
        }
        word.clear();
        dotted = c == Some('.');
        if c.is_none() {
            return false;
        }
    }
}

/// Символ экранирования после `\` (CSS Syntax §4.3.7); `None` — не экранирование.
fn escaped(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<char> {
    let first = *chars.peek()?;
    if first == '\n' || first == '\r' || first == '\u{c}' {
        return None;
    }
    chars.next();
    if !first.is_ascii_hexdigit() {
        return Some(first);
    }
    let mut code = first.to_digit(16)?;
    for _ in 1..6 {
        match chars.peek().and_then(|c| c.to_digit(16)) {
            Some(d) => {
                code = code * 16 + d;
                chars.next();
            }
            None => break,
        }
    }
    if chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
        chars.next();
    }
    Some(
        char::from_u32(code)
            .filter(|_| code != 0)
            .unwrap_or('\u{fffd}'),
    )
}

#[cfg(test)]
mod tests {
    use super::mentions;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn class_selectors_and_escapes_mention_their_class() {
        let alt = names(&["alt"]);
        for css in [
            ".alt{}",
            "html.ALT p{}",
            ":is(.x,.alt){}",
            "p:has(> .alt){}",
            ".\\61 lt{}",
            ".\\000061lt{}",
            "@scope (.alt) {}",
        ] {
            assert!(mentions(css, &[], &alt), "{css}");
        }
        for css in [
            ".alternate{}",
            ".salt{}",
            "alt{}",
            "img[alt]{}",
            "[class~=\"alt\"]{}",
            "\\.alt{}",
            ".a\\lt-x{}",
            "",
        ] {
            assert!(!mentions(css, &[], &alt), "{css}");
        }
    }

    #[test]
    fn attribute_names_are_mentioned_anywhere() {
        let data = names(&["data-edit", "class"]);
        for css in [
            "p::before{content:attr(data-edit)}",
            "[DATA-EDIT]{}",
            "[class~=\"alt\"]{}",
            "/* data-edit */",
            "[data\\-edit]{}",
        ] {
            assert!(mentions(css, &data, &[]), "{css}");
        }
        assert!(!mentions("[data-edits]{}[data]{}.classy{}", &data, &[]));
    }
}
