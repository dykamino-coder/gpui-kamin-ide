//! Attributes the sheets read outside selectors (`attr()`, css-values-5 §7.7).
//!
//! `attr()` substitutes an attribute of THE element its declaration applies
//! to (for a pseudo-element, of its originating element), and only by name.
//! So the dependency of a style on attributes outside selectors is a set of
//! names: an edit of an attribute with another name changes no substitution,
//! and the targeted restyle need not refuse it. Before, any `attr(` in a sheet
//! (even in a comment) sent every edit of the document to a full rebuild.
//!
//! The names come from the text of the sheets, not from parsed rules, so
//! `@keyframes`, `@property` and declarations the parser rejected count too.
//! An extra name (from a comment or a string) only narrows the targeted path;
//! it never makes it wrong. A name the text does not spell
//! (`attr(var(--n))`, an escape) means "any attribute".

/// The names of the attributes the `attr()` calls of the document sheets read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttrReads {
    /// Some `attr()` names its attribute by no literal: any is read.
    any: bool,
    /// Local names, ASCII lowercase (HTML §2.3 compares names case-insensitively).
    names: Vec<String>,
}

impl AttrReads {
    /// Add the names of every `attr()` of the text `css`.
    pub fn scan(&mut self, css: &str) {
        let bytes = css.as_bytes();
        let mut at = 0;
        while let Some(found) = find_attr_call(&bytes[at..]) {
            let args = at + found;
            match attr_name(&css[args..]) {
                Some(name) => {
                    if !self.names.contains(&name) {
                        self.names.push(name);
                    }
                }
                None => self.any = true,
            }
            at = args;
        }
    }

    /// These names with those the `style` attributes `styles` read.
    pub fn with_styles<'a>(&self, styles: impl Iterator<Item = &'a str>) -> Self {
        let mut out = self.clone();
        styles.for_each(|css| out.scan(css));
        out
    }

    /// No `attr()` reads an attribute.
    pub fn is_empty(&self) -> bool {
        !self.any && self.names.is_empty()
    }

    /// Whether some `attr()` reads an attribute of `changed` (ASCII lowercase names).
    pub fn reads_any_of<'a>(&self, mut changed: impl Iterator<Item = &'a str>) -> bool {
        if self.is_empty() {
            return false;
        }
        changed.any(|name| self.any || self.names.iter().any(|n| n.eq_ignore_ascii_case(name)))
    }
}

/// The offset of the first character after `attr(` (the function name is
/// case-insensitive, css-syntax-3 §4.3.4), or `None`.
fn find_attr_call(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(5)
        .position(|w| w[..4].eq_ignore_ascii_case(b"attr") && w[4] == b'(')
        .map(|i| i + 5)
}

/// The local name of the `<attr-name>` at the start of the arguments of
/// `attr()`: `[ <ident>? '|' ]? <ident>`; `None` when it is not a literal.
fn attr_name(args: &str) -> Option<String> {
    let rest = args.trim_start();
    // A string in place of the name (`attr("href")`, as pages write it) is
    // not an `<attr-name>`; if a substitution still takes it, it is that name.
    if let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        let (name, after) = ident(&rest[1..]);
        return (!name.is_empty() && after.starts_with(quote)).then(|| name.to_ascii_lowercase());
    }
    let (first, rest) = ident(rest);
    let rest_trim = rest.trim_start();
    // A namespace prefix (`attr(xlink|href)`, `attr(*|title)`, `attr(|x)`).
    if let Some(after) = rest_trim.strip_prefix('|') {
        let (local, _) = ident(after);
        return (!local.is_empty()).then(|| local.to_ascii_lowercase());
    }
    if first.is_empty() {
        // `attr(*|x)`: the universal prefix.
        let after = rest.strip_prefix('*')?.trim_start().strip_prefix('|')?;
        let (local, _) = ident(after);
        return (!local.is_empty()).then(|| local.to_ascii_lowercase());
    }
    // An escape in the name is not parsed: any attribute.
    if rest.starts_with('\\') {
        return None;
    }
    Some(first.to_ascii_lowercase())
}

/// The leading identifier (without escapes) and the rest.
fn ident(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()))
        .map_or(s.len(), |(i, _)| i);
    // A function (`var(`) is not a name.
    if s[end..].starts_with('(') {
        return ("", s);
    }
    s.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(css: &str) -> AttrReads {
        let mut r = AttrReads::default();
        r.scan(css);
        r
    }

    #[test]
    fn names_of_attr_calls_are_collected_case_insensitively() {
        let r = reads(
            "a::after{content:\" (\" ATTR(Href) \")\"} \
             abbr::before{content:attr( title string, \"?\")} \
             q{--n: attr(data-n type(<number>))} \
             a.x::after{content:attr(\"HREF\")} b::after{content:attr('lang')}",
        );
        assert_eq!(r.names, ["href", "title", "data-n", "lang"]);
        assert!(!r.any);
        assert!(r.reads_any_of(["data-n"].into_iter()));
        assert!(!r.reads_any_of(["class", "style"].into_iter()));
    }

    #[test]
    fn a_namespace_prefix_names_the_local_attribute() {
        assert_eq!(reads("x{content:attr(xlink|href)}").names, ["href"]);
        assert_eq!(reads("x{content:attr(*|title)}").names, ["title"]);
        assert_eq!(reads("x{content:attr(|lang)}").names, ["lang"]);
    }

    #[test]
    fn a_name_that_is_not_a_literal_reads_every_attribute() {
        for css in [
            "x{content:attr(var(--n))}",
            "x{content:attr(\\63 lass)}",
            "x{content:attr()}",
        ] {
            let r = reads(css);
            assert!(r.any, "{css}");
            assert!(r.reads_any_of(["class"].into_iter()), "{css}");
        }
    }

    #[test]
    fn sheets_without_attr_read_nothing() {
        let r = reads("p{color:red} .attr{margin:0} [title]{x:y}");
        assert!(r.is_empty());
        assert!(!r.reads_any_of(["title"].into_iter()));
    }
}
