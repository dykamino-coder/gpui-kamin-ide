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

/// Whether `css` mentions an attribute name of `attrs` anywhere or a class of
/// `classes` after a dot. Attribute names are ASCII lowercase; classes are as
/// written and compared ignoring ASCII case (a superset of the matches).
pub(super) fn mentions(css: &str, attrs: &[String], classes: &[String]) -> bool {
    let mut reads = crate::dom::restyle_probe::AttributeReads::default();
    reads.add(css);
    if reads.reads(attrs) {
        return true;
    }
    scan(css, |word, dotted| {
        attrs.iter().any(|a| a == word)
            || dotted && classes.iter().any(|c| c.eq_ignore_ascii_case(word))
    })
}

/// The identifiers of a set of CSS texts, read in one pass: what `mentions`
/// would find in any of them, so a batch of k elements costs one pass over the
/// sheets instead of k (Blink reads its `RuleFeatureSet` the same way: built
/// once, looked up per mutation).
#[derive(Default)]
pub(super) struct Mentions {
    words: std::collections::HashSet<String>,
    dotted: std::collections::HashSet<String>,
    reads: crate::dom::restyle_probe::AttributeReads,
}

impl Mentions {
    pub(super) fn of<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        let mut index = Self::default();
        for css in texts {
            index.reads.add(css);
            scan(css, |word, dotted| {
                if dotted && !index.dotted.contains(word) {
                    index.dotted.insert(word.to_string());
                }
                if !index.words.contains(word) {
                    index.words.insert(word.to_string());
                }
                false
            });
        }
        index
    }

    /// `mentions` over every indexed text: the words are kept in ASCII
    /// lowercase, so a class written in any case is looked up lowercased
    /// (the same superset of selector matches as `mentions`).
    pub(super) fn any(&self, attrs: &[String], classes: &[String]) -> bool {
        self.reads.reads(attrs)
            || attrs.iter().any(|a| self.words.contains(a))
            || (classes.iter()).any(|c| self.dotted.contains(&c.to_ascii_lowercase()))
    }
}

/// Calls `found` with each identifier of `css` (ASCII lowercase, escapes
/// resolved) and whether an unescaped dot precedes it; stops at `true`.
fn scan(css: &str, mut found: impl FnMut(&str, bool) -> bool) -> bool {
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
        if !word.is_empty() && found(&word, dotted) {
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

    #[test]
    fn a_class_in_any_case_is_mentioned_by_the_index_as_by_each_sheet() {
        // The changed classes of `A` -> `a` are both as written (#1073): the
        // index finds them as `mentions` does, ignoring ASCII case.
        let changed = names(&["A", "a"]);
        for css in [".A{color:red}", ".a{color:blue}", ".\\41 {}"] {
            assert!(mentions(css, &[], &changed), "{css}");
            assert!(super::Mentions::of([css]).any(&[], &changed), "{css}");
        }
        let upper = names(&["ALT"]);
        assert!(super::Mentions::of([".alt{}"]).any(&[], &upper));
        assert!(!super::Mentions::of([".salt{}"]).any(&[], &upper));
    }

    #[test]
    fn the_index_of_several_sheets_finds_what_each_sheet_mentions() {
        let sheets = [
            ".alt{}",
            "p::before{content:attr(data-edit)}",
            "/* data-x */",
            ".\\61 b{}",
        ];
        let index = super::Mentions::of(sheets);
        for (attrs, classes) in [
            (vec![], names(&["alt"])),
            (vec![], names(&["ab"])),
            (names(&["data-edit"]), vec![]),
            (names(&["data-x"]), vec![]),
            (vec![], names(&["data-x"])),
            (names(&["alt"]), vec![]),
            (names(&["data-y"]), names(&["salt", "edit"])),
        ] {
            let each = sheets.iter().any(|css| mentions(css, &attrs, &classes));
            assert_eq!(index.any(&attrs, &classes), each, "{attrs:?} {classes:?}");
        }
    }

    /// `cargo test --release -p kamin-html --lib mentions_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    #[allow(clippy::print_stderr)]
    fn mentions_cost_of_a_batch() {
        let ua = crate::style::ua::user_agent_css();
        let sheet: String = (0..2000)
            .map(|i| format!(".c{i} p[data-k{i}]{{color:red}}"))
            .collect();
        let texts = [ua, sheet.as_str()];
        let attrs: Vec<Vec<String>> = (0..500).map(|i| vec![format!("data-z{i}")]).collect();
        let start = std::time::Instant::now();
        let each = attrs
            .iter()
            .filter(|a| texts.iter().any(|css| mentions(css, a, &[])))
            .count();
        let per_element = start.elapsed();
        let start = std::time::Instant::now();
        let index = super::Mentions::of(texts);
        let once = attrs.iter().filter(|a| index.any(a, &[])).count();
        let indexed = start.elapsed();
        assert_eq!(each, once);
        eprintln!(
            "500 elements, {} bytes: per element {per_element:?}, index {indexed:?}",
            ua.len() + sheet.len()
        );
    }
}
