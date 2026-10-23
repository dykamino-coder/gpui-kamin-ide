//! Разрывы и переносы: `overflow-wrap`, `word-break`, `line-break` (css-text-4
//! §5.2-5.5), `hyphens`, `hyphenate-character`, `hyphenate-limit-chars`,
//! `hyphenate-limit-lines` (§6). Каждое свойство — свои поля: использованное
//! значение пары `word-break: break-word` / `overflow-wrap` складывает
//! читатель.

use super::hyphen_limits::parse_hyphenate_limit_chars;
use crate::style::computed::{Computed, unescape_content};
use crate::style::css::LonghandId;

/// `overflow-wrap: normal | break-word | anywhere`: (рвать слово, `anywhere`).
pub(crate) fn overflow_wrap(v: &str) -> Option<(bool, bool)> {
    match v.to_ascii_lowercase().as_str() {
        "normal" => Some((false, false)),
        "break-word" => Some((true, false)),
        "anywhere" => Some((true, true)),
        _ => None,
    }
}

/// `word-break: normal | break-all | keep-all | auto-phrase | break-word`
/// (устаревшее `break-word` = `normal` + `overflow-wrap: anywhere`, §5.2).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordBreakKind {
    Normal,
    BreakAll,
    KeepAll,
    AutoPhrase,
    BreakWord,
}

pub(crate) fn word_break(v: &str) -> Option<WordBreakKind> {
    Some(match v.to_ascii_lowercase().as_str() {
        "normal" => WordBreakKind::Normal,
        "break-all" => WordBreakKind::BreakAll,
        "keep-all" => WordBreakKind::KeepAll,
        "auto-phrase" => WordBreakKind::AutoPhrase,
        "break-word" => WordBreakKind::BreakWord,
        _ => return None,
    })
}

/// `line-break: auto | loose | normal | strict | anywhere`: (`anywhere`,
/// послабление CJK: 1 `normal`, 2 `loose`, иначе 0).
pub(crate) fn line_break(v: &str) -> Option<(bool, u8)> {
    Some(match v.to_ascii_lowercase().as_str() {
        "auto" | "strict" => (false, 0),
        "normal" => (false, 1),
        "loose" => (false, 2),
        "anywhere" => (true, 0),
        _ => return None,
    })
}

/// `hyphens: none | manual | auto`: (переносить, расставлять самим).
pub(crate) fn hyphens(v: &str) -> Option<(bool, bool)> {
    match v.to_ascii_lowercase().as_str() {
        "none" => Some((false, false)),
        "manual" => Some((true, false)),
        "auto" => Some((true, true)),
        _ => None,
    }
}

/// `hyphenate-character: auto | <string>`.
pub(crate) fn hyphenate_character(v: &str) -> Option<String> {
    if v.eq_ignore_ascii_case("auto") {
        return Some("\u{2010}".into());
    }
    let q = v.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    (v.len() >= 2 && v.ends_with(q)).then(|| unescape_content(&v[1..v.len() - 1]))
}

/// `hyphenate-limit-lines: no-limit | <integer [0,∞]>`.
pub(crate) fn limit_lines(v: &str) -> Option<usize> {
    if v.eq_ignore_ascii_case("no-limit") {
        return Some(usize::MAX);
    }
    v.parse().ok()
}

pub(super) fn accepts(id: LonghandId, v: &str) -> Option<bool> {
    use LonghandId::*;
    Some(match id {
        OverflowWrap => overflow_wrap(v).is_some(),
        WordBreak => word_break(v).is_some(),
        LineBreak => line_break(v).is_some(),
        Hyphens => hyphens(v).is_some(),
        HyphenateCharacter => hyphenate_character(v).is_some(),
        HyphenateLimitChars => parse_hyphenate_limit_chars(v).is_some(),
        HyphenateLimitLines => limit_lines(v).is_some(),
        _ => return None,
    })
}

impl Computed {
    pub(super) fn apply_breaking(&mut self, id: LonghandId, v: &str) -> bool {
        use LonghandId::*;
        match id {
            OverflowWrap => {
                if let Some((word, anywhere)) = overflow_wrap(v) {
                    self.break_word = Some(word);
                    self.wrap_anywhere = Some(anywhere);
                }
            }
            WordBreak => {
                if let Some(kind) = word_break(v) {
                    self.break_anywhere = Some(kind == WordBreakKind::BreakAll);
                    self.keep_all = Some(kind == WordBreakKind::KeepAll);
                    self.auto_phrase = Some(kind == WordBreakKind::AutoPhrase);
                    self.word_break_word = Some(kind == WordBreakKind::BreakWord);
                }
            }
            LineBreak => {
                if let Some((anywhere, loose)) = line_break(v) {
                    self.break_anywhere_strict = Some(anywhere);
                    self.line_break_loose = Some(loose);
                }
            }
            Hyphens => {
                if let Some((on, auto)) = hyphens(v) {
                    self.hyphenate = Some(on);
                    self.hyphens_auto = Some(auto);
                }
            }
            HyphenateCharacter => {
                self.hyphen_char = hyphenate_character(v).or(self.hyphen_char.take())
            }
            HyphenateLimitChars => {
                self.hyphen_limit_chars = parse_hyphenate_limit_chars(v).or(self.hyphen_limit_chars)
            }
            HyphenateLimitLines => {
                self.hyphen_limit_lines = limit_lines(v).or(self.hyphen_limit_lines)
            }
            _ => return false,
        }
        true
    }

    pub(super) fn inherit_breaking(&mut self, id: LonghandId) -> bool {
        use LonghandId::*;
        match id {
            OverflowWrap => (self.break_word, self.wrap_anywhere) = (None, None),
            WordBreak => {
                self.break_anywhere = None;
                self.keep_all = None;
                self.auto_phrase = None;
                self.word_break_word = None;
            }
            LineBreak => (self.break_anywhere_strict, self.line_break_loose) = (None, None),
            Hyphens => (self.hyphenate, self.hyphens_auto) = (None, None),
            HyphenateCharacter => self.hyphen_char = None,
            HyphenateLimitChars => self.hyphen_limit_chars = None,
            HyphenateLimitLines => self.hyphen_limit_lines = None,
            _ => return false,
        }
        true
    }
}
