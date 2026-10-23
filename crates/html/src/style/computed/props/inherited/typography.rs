//! Типографика строки: `word-space-transform` (css-text-4 §5.3),
//! `text-autospace` (§10.1), `hanging-punctuation` (§9.2), `text-fit`
//! (css-text-5).

use crate::style::computed::{Computed, Hanging, TextFit};
use crate::style::css::LonghandId;

/// Слова без повторов из набора `allowed`.
fn unique(words: &[&str], allowed: &[&str]) -> bool {
    !words.is_empty()
        && words
            .iter()
            .enumerate()
            .all(|(i, w)| allowed.contains(w) && !words[..i].contains(w))
}

/// `word-space-transform: none | [space | ideographic-space] &&
/// auto-phrase?`: знак замены (`\0` — без замены).
pub(crate) fn word_space_transform(v: &str) -> Option<char> {
    let lower = v.to_ascii_lowercase();
    let words: Vec<_> = lower.split_whitespace().collect();
    if words == ["none"] {
        return Some('\0');
    }
    if !unique(&words, &["space", "ideographic-space", "auto-phrase"]) {
        return None;
    }
    let mut marks = words.iter().filter_map(|w| match *w {
        "space" => Some(' '),
        "ideographic-space" => Some('\u{3000}'),
        _ => None,
    });
    let mark = marks.next()?;
    marks.next().is_none().then_some(mark)
}

/// `text-autospace: normal | auto | no-autospace | [ideograph-alpha ||
/// ideograph-numeric || punctuation] || [insert | replace]`: (буквы,
/// цифры).
pub(crate) fn autospace(v: &str) -> Option<(bool, bool)> {
    let lower = v.to_ascii_lowercase();
    match lower.as_str() {
        "normal" | "auto" => return Some((true, true)),
        "no-autospace" => return Some((false, false)),
        _ => {}
    }
    let words: Vec<_> = lower.split_whitespace().collect();
    let allowed = [
        "ideograph-alpha",
        "ideograph-numeric",
        "punctuation",
        "insert",
        "replace",
    ];
    (unique(&words, &allowed) && !(words.contains(&"insert") && words.contains(&"replace"))).then(
        || {
            (
                words.contains(&"ideograph-alpha"),
                words.contains(&"ideograph-numeric"),
            )
        },
    )
}

/// `hanging-punctuation: none | [first || [force-end | allow-end] || last]`.
pub(crate) fn hanging(v: &str) -> Option<Hanging> {
    let lower = v.to_ascii_lowercase();
    let words: Vec<_> = lower.split_whitespace().collect();
    if words == ["none"] {
        return Some(Hanging::default());
    }
    let end = words.contains(&"force-end") && words.contains(&"allow-end");
    (unique(&words, &["first", "last", "force-end", "allow-end"]) && !end).then(|| Hanging {
        first: words.contains(&"first"),
        last: words.contains(&"last"),
        force_end: words.contains(&"force-end"),
        allow_end: words.contains(&"allow-end"),
    })
}

/// `text-fit: [none | grow | shrink] [consistent | per-line | per-line-all]?
/// <percentage [0,∞]>?` (css-text-5): `None` — без подгонки.
pub(crate) fn text_fit(v: &str) -> Option<Option<TextFit>> {
    let lower = v.to_ascii_lowercase();
    let mut words = lower.split_whitespace().peekable();
    let (grow, shrink) = match words.next()? {
        "none" => (false, false),
        "grow" => (true, false),
        "shrink" => (false, true),
        _ => return None,
    };
    let (per_line, all) = match words.peek().copied() {
        Some("consistent") => (false, false),
        Some("per-line") => (true, false),
        Some("per-line-all") => (true, true),
        _ => (false, false),
    };
    if matches!(
        words.peek().copied(),
        Some("consistent" | "per-line" | "per-line-all")
    ) {
        words.next();
    }
    let target = match words.next() {
        Some(w) => Some(
            w.strip_suffix('%')?
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0.0)?
                / 100.0,
        ),
        None => None,
    };
    if words.next().is_some() {
        return None;
    }
    Some((grow || shrink).then_some(TextFit {
        grow,
        shrink,
        per_line,
        all,
        target,
    }))
}

pub(super) fn accepts(id: LonghandId, v: &str) -> Option<bool> {
    use LonghandId::*;
    Some(match id {
        WordSpaceTransform => word_space_transform(v).is_some(),
        TextAutospace => autospace(v).is_some(),
        HangingPunctuation => hanging(v).is_some(),
        TextFit => text_fit(v).is_some(),
        _ => return None,
    })
}

impl Computed {
    pub(super) fn apply_typography(&mut self, id: LonghandId, v: &str) -> bool {
        use LonghandId::*;
        match id {
            WordSpaceTransform => {
                self.word_space_char = word_space_transform(v).or(self.word_space_char)
            }
            TextAutospace => {
                if let Some((alpha, numeric)) = autospace(v) {
                    self.autospace_alpha = Some(alpha);
                    self.autospace_numeric = Some(numeric);
                }
            }
            HangingPunctuation => self.hanging = hanging(v).or(self.hanging),
            TextFit => {
                if let Some(fit) = text_fit(v) {
                    self.text_fit = Some(fit);
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn inherit_typography(&mut self, id: LonghandId) -> bool {
        use LonghandId::*;
        match id {
            WordSpaceTransform => self.word_space_char = None,
            TextAutospace => (self.autospace_alpha, self.autospace_numeric) = (None, None),
            HangingPunctuation => self.hanging = None,
            TextFit => self.text_fit = None,
            _ => return false,
        }
        true
    }
}
