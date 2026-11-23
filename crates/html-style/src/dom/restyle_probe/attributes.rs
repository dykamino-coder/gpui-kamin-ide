//! Attribute dependencies by name preserve local cascade for unrelated edits.

use crate::css_syntax::{escape_end, functions, strip_comments, unescape};
use std::collections::HashSet;

#[derive(Default)]
/// Attributes read by CSS values; typed reads can introduce dynamic dependencies.
pub struct AttributeReads {
    names: HashSet<String>,
    dynamic: bool,
}

impl AttributeReads {
    /// Add dependencies from a stylesheet or an element's own style.
    pub fn add(&mut self, css: &str) {
        let clean;
        let css = if css.contains("/*") {
            clean = strip_comments(css);
            &clean
        } else {
            css
        };
        for call in functions(css).filter(|f| f.name.eq_ignore_ascii_case("attr")) {
            let body = css[call.open + 1..].trim_start();
            let mut end = 0;
            while let Some(c) = body[end..].chars().next() {
                if c == '\\' {
                    end = escape_end(body, end);
                } else if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '|') || c >= '\u{80}'
                {
                    end += c.len_utf8();
                } else {
                    break;
                }
            }
            let name = unescape(&body[..end]);
            let name = name.strip_prefix('|').unwrap_or(&name);
            let tail = body[end..].trim_start();
            let tail = match tail.get(..10) {
                Some(raw) if raw.eq_ignore_ascii_case("raw-string") => tail[10..].trim_start(),
                _ => tail,
            };
            // Typed attr() expands CSS from the attribute, including nested attr().
            // Unknown names and syntax require conservative invalidation.
            if name.is_empty() || name.contains('|') || !tail.starts_with([',', ')']) {
                self.dynamic = true;
            } else {
                self.names.insert(name.to_ascii_lowercase());
            }
        }
    }

    /// Whether editing these names can change computed values.
    pub fn reads(&self, names: &[String]) -> bool {
        self.dynamic || names.iter().any(|name| self.names.contains(name))
    }
}

#[cfg(test)]
mod tests {
    use super::AttributeReads;

    fn reads(css: &str, name: &str) -> bool {
        let mut reads = AttributeReads::default();
        reads.add(css);
        reads.reads(&[name.into()])
    }

    #[test]
    fn raw_attributes_only_invalidate_the_names_they_read() {
        let css = "p::after{content:attr(title) attr(data-label raw-string)}";
        assert!(reads(css, "title"));
        assert!(reads(css, "data-label"));
        assert!(!reads(css, "class"));
        assert!(!reads(css, "data-edit"));
    }

    #[test]
    fn lexical_boundaries_escapes_case_and_empty_namespace_are_kept() {
        for css in [
            r"p{content:a\74tr(\64 ata-X)}",
            "p{content:ATTR(/*x*/ DATA-X/**/ RAW-STRING)}",
            "p{content:attr(|data-x)}",
        ] {
            assert!(reads(css, "data-x"), "{css}");
            assert!(!reads(css, "class"), "{css}");
        }
        for css in [
            "p{content:'attr(class)'}",
            "/* attr(class) */ p{color:red}",
            "p{background:url(attr(class))}",
            "p{content:fooattr(class)}",
            "#attr(class){} @attr(class);",
        ] {
            assert!(!reads(css, "class"), "{css}");
        }
    }

    #[test]
    fn fallbacks_are_indexed_and_dynamic_reads_stay_conservative() {
        let css = "p{content:attr(title, attr(data-label))}";
        assert!(reads(css, "title"));
        assert!(reads(css, "data-label"));
        assert!(!reads(css, "class"));
        for css in [
            "p{width:attr(data-x px)}",
            "p{color:attr(data-x type(*))}",
            "p{content:attr(var(--name))}",
            "p{content:attr(ns|data-x)}",
            "p{content:attr(}",
        ] {
            assert!(reads(css, "class"), "{css}");
        }
    }
}
