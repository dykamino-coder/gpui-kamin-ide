//! Paged (print) reftest detection: which pairs are rendered as a page stack.
//!
//! WPT `docs/writing-tests/print-reftests.md`: a print reftest is a test whose
//! file name ends in `-print` (before the extension), or whose root element
//! carries the `reftest-paged` class (the Gecko convention). Both the test and
//! its reference are then paginated with the default page box (5in x 3in,
//! 0.5in margins) and compared page by page. Being paged is a property of the
//! PAIR: the reference of a `-print` test is printed the same way even when its
//! own name does not say so (`page-name-001-print` vs `page-name-001-ref`).

/// Is this file a paged reftest document (by name or by `reftest-paged`)?
pub(super) fn is_paged(path: &str, html: &str) -> bool {
    paged_name(path) || root_has_paged_class(html)
}

/// `foo-print.html`, `foo-print.tentative.html`, `foo-print-ref.html`.
fn paged_name(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.contains("-print.") || name.contains("-print-ref")
}

/// `<html class="... reftest-paged ...">` on the root element.
fn root_has_paged_class(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<html") {
        let start = from + at;
        let after = lower.as_bytes().get(start + 5).copied();
        from = start + 5;
        // `<html>` / `<html ...>` only, not `<htmlfoo>`.
        if !matches!(after, Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
            continue;
        }
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        let tag = &lower[start..end];
        return super::html_attrs::attr_value(tag, "class")
            .is_some_and(|c| c.split_ascii_whitespace().any(|t| t == "reftest-paged"));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_paged;

    #[test]
    fn print_suffix_marks_test_and_reference() {
        assert!(is_paged(r"C:\wpt\css\css-page\page-box-001-print.html", ""));
        assert!(is_paged("css/css-page/a-print.tentative.html", ""));
        assert!(is_paged("css/css-page/a-print-ref.html", ""));
        assert!(!is_paged("css/css-page/a-ref.html", ""));
        assert!(!is_paged("css/css-print-x/a.html", ""));
    }

    #[test]
    fn reftest_paged_class_on_root() {
        assert!(is_paged(
            "a.html",
            "<!doctype html><html class=\"reftest-paged\">"
        ));
        assert!(is_paged("a.html", "<HTML lang=en class='x reftest-paged'>"));
        assert!(!is_paged("a.html", "<html class=\"reftest-wait\">"));
        assert!(!is_paged("a.html", "<div class=\"reftest-paged\"></div>"));
        assert!(!is_paged("a.html", "<htmlx class=\"reftest-paged\">"));
    }
}
