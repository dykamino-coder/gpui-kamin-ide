//! Resolved visual runs must not undergo a second Unicode bidi analysis in the shaper.
//! CSS Writing Modes 4 §2.4 and UAX #9 L4: mirror once at the resolved run level.
use gpui::{SharedString, TextRun};

pub(super) fn text(body: &str, runs: &mut [TextRun], rtl: bool) -> SharedString {
    // GPUI exports glyphs in logical order and reverses RTL clusters separately.
    // An explicit override keeps the platform shaper from resolving levels again:
    // an LRO for a left-to-right run (no RTL behavior inside a CSS left-to-right
    // override), an RLO for a resolved right-to-left run. Under an LRO the shaper
    // shaped Arabic as a left-to-right run and lost its glyphs next to ZWNJ and
    // tatweel (shaping-no-join-*, shaping-tatweel-001). Under the RLO every
    // character of the run takes the run's odd level, so the shaper mirrors the
    // paired punctuation itself, exactly once (UAX #9 L4), and the text is passed
    // unmirrored.
    let open = if rtl { '\u{202e}' } else { '\u{202d}' };
    const CLOSE: char = '\u{202c}';
    if let Some(first) = runs.first_mut() {
        first.len += open.len_utf8();
    }
    if let Some(last) = runs.last_mut() {
        last.len += CLOSE.len_utf8();
    }
    let mut out = String::with_capacity(body.len() + 6);
    out.push(open);
    out.push_str(body);
    out.push(CLOSE);
    out.into()
}
