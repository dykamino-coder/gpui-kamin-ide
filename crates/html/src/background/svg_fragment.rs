//! Resolve SVG image URL fragments before passing the image to static usvg.

pub(super) fn resolve(bytes: Vec<u8>, id: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(doc) = roxmltree::Document::parse_with_options(text, options) else {
        return bytes;
    };
    let decoded = super::percent_decode(id);
    let Ok(id) = std::str::from_utf8(&decoded) else {
        return bytes;
    };
    let Some(target) = doc
        .descendants()
        .find(|n| n.is_element() && n.attribute("id") == Some(id))
    else {
        return bytes;
    };
    let mut edits = Vec::new();
    let root = doc.root_element();
    // SVG 2 section 16.3.2: a <view> fragment supplies the initial viewBox.
    if target.has_tag_name("view") {
        if let Some(value) = target.attribute("viewBox") {
            let value = xml_text(value);
            if let Some(attr) = root.attributes().find(|a| a.name() == "viewBox") {
                edits.push((attr.range_value(), value));
            } else if let Some(end) = opening_end(text, root.range().start) {
                edits.push((end..end, format!(" viewBox=\"{value}\"")));
            }
        }
    }
    // Selectors 4 section 8.3: the fragment's first matching element is :target.
    // usvg has no document URL. An unused attribute marks just that element;
    // an attribute selector has the same specificity as this pseudo-class.
    let mut marker = "data-kamin-svg-target".to_string();
    while doc
        .descendants()
        .any(|n| n.attribute(marker.as_str()).is_some())
    {
        marker.push('_');
    }
    let selector = format!("[{marker}]");
    let mut changed = false;
    for node in doc.descendants().filter(|n| n.has_tag_name("style")) {
        if node.attribute("type").is_some_and(|t| t != "text/css") {
            continue;
        }
        let css: String = node
            .children()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .collect();
        let rewritten = target_selectors(&css, &selector);
        if css == rewritten {
            continue;
        }
        let range = node.range();
        if let (Some(start), Some(end)) = (
            opening_end(text, range.start),
            text[range.clone()].rfind("</"),
        ) {
            edits.push((start + 1..range.start + end, xml_text(&rewritten)));
            changed = true;
        }
    }
    if changed {
        if let Some(end) = opening_end(text, target.range().start) {
            let at = if text.as_bytes()[end - 1] == b'/' {
                end - 1
            } else {
                end
            };
            edits.push((at..at, format!(" {marker}=\"\"")));
        }
    }
    if edits.is_empty() {
        return bytes;
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut result = text.to_string();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range, &value);
    }
    result.into_bytes()
}

fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn opening_end(text: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (i, c) in text[start..].char_indices() {
        if quote == Some(c) {
            quote = None;
        } else if quote.is_none() {
            if c == '>' {
                return Some(start + i);
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
            }
        }
    }
    None
}

/// Rewrite selector tokens only: comments, strings, attributes and declarations
/// may contain the literal text :target and must retain their original value.
fn target_selectors(css: &str, selector: &str) -> String {
    let b = css.as_bytes();
    let mut out = String::new();
    let (mut i, mut from, mut head, mut brackets) = (0, 0, 0, 0usize);
    let mut declarations = vec![false];
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                i += 1;
            }
            i = (i + 2).min(b.len());
            continue;
        }
        if b[i] == b'\'' || b[i] == b'"' {
            let quote = b[i];
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' {
                    i = (i + 2).min(b.len());
                } else if b[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        if b[i] == b'\\' {
            i = name_end(b, i);
            continue;
        }
        match b[i] {
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            b'{' if brackets == 0 => {
                let prelude = crate::css::strip_comments(&css[head..i]);
                let name = prelude.trim().split_whitespace().next().unwrap_or("");
                let group = [
                    "@media",
                    "@supports",
                    "@layer",
                    "@container",
                    "@scope",
                    "@document",
                ]
                .iter()
                .any(|v| name.eq_ignore_ascii_case(v));
                declarations.push(*declarations.last().unwrap() || !group);
                head = i + 1;
            }
            b'}' if brackets == 0 => {
                if declarations.len() > 1 {
                    declarations.pop();
                }
                head = i + 1;
            }
            b';' if brackets == 0 => head = i + 1,
            b':' if brackets == 0
                && !declarations.last().unwrap()
                && (i == 0 || b[i - 1] != b':')
                && b.get(i + 1) != Some(&b':') =>
            {
                let end = name_end(b, i + 1);
                if crate::css::unescape(&css[i + 1..end]).eq_ignore_ascii_case("target")
                    && b.get(end) != Some(&b'(')
                {
                    out.push_str(&css[from..i]);
                    out.push_str(selector);
                    from = end;
                }
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out.push_str(&css[from..]);
    out
}

fn name_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        if b[i] == b'\\' {
            i += 1;
            let start = i;
            while i < b.len() && i - start < 6 && b[i].is_ascii_hexdigit() {
                i += 1;
            }
            if i == start {
                i = (i + 1).min(b.len());
            } else if i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
        } else if b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b'_' || b[i] >= 128 {
            i += 1;
        } else {
            break;
        }
    }
    i
}
