//! Resolved visual runs must not undergo a second Unicode bidi analysis in the shaper.
//! CSS Writing Modes 4 §2.4 and UAX #9 L4: mirror once at the resolved run level.
use gpui::{SharedString, TextRun};

pub(super) fn text(body: &str, runs: &mut [TextRun], rtl: bool) -> SharedString {
    // GPUI exports glyphs in logical order and reverses RTL clusters separately.
    // An LRO keeps the platform shaper from mirroring Hebrew-adjacent punctuation
    // again, or reintroducing RTL behavior inside a CSS left-to-right override.
    const OPEN: char = '\u{202d}';
    const CLOSE: char = '\u{202c}';
    if let Some(first) = runs.first_mut() {
        first.len += OPEN.len_utf8();
    }
    if let Some(last) = runs.last_mut() {
        last.len += CLOSE.len_utf8();
    }
    let mut out = String::with_capacity(body.len() + 6);
    out.push(OPEN);
    out.extend(body.chars().map(|ch| if rtl { mirror(ch) } else { ch }));
    out.push(CLOSE);
    out.into()
}

fn mirror(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        other => other,
    }
}
