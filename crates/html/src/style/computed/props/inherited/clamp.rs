//! Обрезка блока и край текста: `max-lines`, `block-ellipsis`, `continue`
//! (css-overflow-4 §5, редакция WPT `css-overflow/parsing` и Blink
//! `AlternativeLineClampShorthand`; их сокращения `line-clamp`,
//! `-webkit-line-clamp` раскрыты разбором), `text-box-trim`, `text-box-edge`
//! (css-inline-3 §4; сокращение `text-box`). Наследуются только
//! `block-ellipsis` и `text-box-edge`.

use super::text_box_values::{parse_edge, parse_trim};
use crate::style::computed::{Computed, collapse_forced_breaks, unescape_content};
use crate::style::css::LonghandId;

/// `max-lines: <integer [1,∞]> || auto`: (предел строк, `auto` — срез и по
/// высоте коробки).
pub(crate) fn max_lines(v: &str) -> Option<(Option<u32>, bool)> {
    let (mut lines, mut auto) = (None, false);
    for word in v.split_whitespace() {
        if word.eq_ignore_ascii_case("auto") && !auto {
            auto = true;
        } else if lines.is_none() {
            lines = Some(word.parse::<u32>().ok().filter(|n| *n >= 1)?);
        } else {
            return None;
        }
    }
    (lines.is_some() || auto).then_some((lines, auto))
}

/// `block-ellipsis: no-ellipsis | ellipsis | <string>`: `None` — `ellipsis`
/// (знак U+2026), пустая строка — `no-ellipsis`.
pub(crate) fn block_ellipsis(v: &str) -> Option<Option<String>> {
    match v.to_ascii_lowercase().as_str() {
        "ellipsis" => return Some(None),
        "no-ellipsis" => return Some(Some(String::new())),
        _ => {}
    }
    let q = v.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let len = crate::style::css::skip_string(&v[1..], q);
    (v[1..1 + len].ends_with(q) && 1 + len == v.len()).then(|| {
        Some(collapse_forced_breaks(&unescape_content(
            &v[1..v.len() - 1],
        )))
    })
}

/// `continue: normal | collapse | -webkit-legacy` (`discard` не поддержан —
/// как у Blink): (`collapse`, `-webkit-legacy`).
pub(crate) fn continue_value(v: &str) -> Option<(bool, bool)> {
    match v.to_ascii_lowercase().as_str() {
        "normal" => Some((false, false)),
        "collapse" => Some((true, false)),
        "-webkit-legacy" => Some((false, true)),
        _ => None,
    }
}

pub(super) fn accepts(id: LonghandId, v: &str) -> Option<bool> {
    use LonghandId::*;
    Some(match id {
        MaxLines => max_lines(v).is_some(),
        BlockEllipsis => block_ellipsis(v).is_some(),
        Continue => continue_value(v).is_some(),
        TextBoxTrim => parse_trim(&v.to_ascii_lowercase()).is_some(),
        TextBoxEdge => parse_edge(&v.to_ascii_lowercase()).is_some(),
        _ => return None,
    })
}

impl Computed {
    pub(super) fn apply_clamp(&mut self, id: LonghandId, v: &str) -> bool {
        use LonghandId::*;
        match id {
            MaxLines => {
                if let Some((lines, auto)) = max_lines(v) {
                    self.line_clamp = lines;
                    self.clamp_auto = Some(auto);
                }
            }
            BlockEllipsis => {
                if let Some(mark) = block_ellipsis(v) {
                    self.clamp_mark = Some(mark);
                }
            }
            Continue => {
                if let Some((collapse, legacy)) = continue_value(v) {
                    self.clamp_continue = Some(collapse);
                    self.clamp_legacy = Some(legacy);
                }
            }
            TextBoxTrim => {
                if let Some((start, end)) = parse_trim(&v.to_ascii_lowercase()) {
                    (self.text_box_trim_start, self.text_box_trim_end) = (start, end);
                }
            }
            TextBoxEdge => {
                if let Some((over, under)) = parse_edge(&v.to_ascii_lowercase()) {
                    self.text_box_edge_set = true;
                    (self.text_box_over, self.text_box_under) = (over, under);
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn inherit_clamp(&mut self, id: LonghandId) -> bool {
        match id {
            LonghandId::BlockEllipsis => self.clamp_mark = None,
            LonghandId::TextBoxEdge => self.text_box_edge_set = false,
            _ => return false,
        }
        true
    }
}
