//! Разбиение на строки: split, lay_in, балансировка, аварийные разрывы, места переноса (UAX #14).

mod cluster_edges;
mod punctuation;
pub(super) use crate::text::paragraph::breaking::cluster_edges::cluster_edge;
use crate::text::paragraph::breaking::cluster_edges::cluster_edge_at;
use crate::text::paragraph::breaking::punctuation::centered_punctuation;
use crate::text::paragraph::breaking::punctuation::conditional_japanese_starter;
pub(super) use crate::text::paragraph::breaking::punctuation::is_closing;
pub(super) use crate::text::paragraph::breaking::punctuation::is_opening;
pub(super) use crate::text::paragraph::breaking::punctuation::is_stop;
use crate::text::paragraph::breaking::punctuation::iteration_mark;
use crate::text::paragraph::breaking::punctuation::letter_unit;
use crate::text::paragraph::breaking::punctuation::no_break_after;
use crate::text::paragraph::breaking::punctuation::no_break_before;
use crate::text::paragraph::breaking::punctuation::wide_postfix;
use crate::text::paragraph::breaking::punctuation::wide_prefix;

mod cut;
mod layout_lines;
mod opportunities;
mod soft_opportunities;
mod split_lines;
mod unicode_breaks;
mod wrap_policy;

use crate::text::paragraph::*;

/// Правила переноса из стиля БЕЗ вопроса, нужна ли своя раскладка.
pub fn wrap_of(c: &crate::style::computed::Computed) -> Wrap {
    Wrap {
        nowrap: c.nowrap == Some(true),
        break_spaces: c.break_after_spaces == Some(true),
        break_all: c.break_anywhere == Some(true) && c.break_anywhere_strict != Some(true),
        anywhere: c.break_anywhere_strict == Some(true),
        keep_all: c.keep_all == Some(true),
        auto_phrase: c.auto_phrase == Some(true),
        break_word: c.break_word == Some(true),
        wrap_anywhere: c.wrap_anywhere == Some(true),
        rtl: c.wm().rtl(),
        balance: c.balance_lines == Some(true),
        keep_spaces: c.keep_spaces == Some(true),
        loose: c.line_break_loose.unwrap_or(0),
        cjk_lang: c.lang.as_deref().is_some_and(cjk_writing),
    }
}

/// Chinese or Japanese content for `line-break` (css-text-3 §5.3): the
/// language itself, or a Chinese/Japanese script subtag (BCP 47) even under
/// another language (`writing-system-line-break-002`: `en-Hrkt`).
fn cjk_writing(lang: &str) -> bool {
    let mut subtags = lang.split(['-', '_']);
    let language = subtags.next().unwrap_or_default();
    if let Some(script) =
        subtags.find(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()))
    {
        return ["hrkt", "hira", "kana", "jpan", "hani", "hans", "hant"]
            .iter()
            .any(|s| script.eq_ignore_ascii_case(s));
    }
    language.eq_ignore_ascii_case("ja") || language.eq_ignore_ascii_case("zh")
}

#[cfg(test)]
mod tests;
