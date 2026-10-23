//! Интервалы и выравнивание строк: `letter-spacing`, `word-spacing`
//! (css-text-4 §8), `text-indent` (§7.1), `text-align-all`, `text-align-last`
//! (§7.2-7.3; их сокращение `text-align` раскрыто разбором), `text-justify`
//! (§7.4), `text-transform` (§2.1), `tab-size` (§4.2).

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, TT_FULL_WIDTH, TT_KANA, TT_MATH, TextAlign, TextTransform};
use crate::style::css::LonghandId;
use crate::style::values::value::{Len, css_number};

/// Отступ первой строки: длина, `each-line`, `hanging`.
pub(crate) fn text_indent(v: &str, vcx: ValueCx<'_>) -> Option<(Len, bool, bool)> {
    let (mut length, mut each, mut hanging) = (None, false, false);
    for word in crate::style::computed::split_outside_parens(v) {
        match word.to_ascii_lowercase().as_str() {
            "each-line" if !each => each = true,
            "hanging" if !hanging => hanging = true,
            _ if length.is_none() => {
                length = Some(Len::parse_mixed(&word, vcx).filter(|l| {
                    !matches!(
                        l,
                        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                    )
                })?)
            }
            _ => return None,
        }
    }
    Some((length?, each, hanging))
}

/// Выравнивание строк: `None` — `match-parent`; у `text-align-last` слово
/// `auto` — `Some(None)`.
pub(crate) fn align(v: &str, last: bool) -> Option<Option<Option<TextAlign>>> {
    Some(Some(Some(match v.to_ascii_lowercase().as_str() {
        "match-parent" => return Some(None),
        "auto" if last => return Some(Some(None)),
        "start" => TextAlign::Start,
        "end" => TextAlign::End,
        "left" => TextAlign::Left,
        "right" => TextAlign::Right,
        "center" => TextAlign::Center,
        "justify" => TextAlign::Justify,
        _ => return None,
    })))
}

/// `text-justify: auto | none | inter-word | inter-character` (и устаревшее
/// `distribute` = `inter-character`, css-text-4 §7.4; `ruby`): (запрет
/// выключки, растягивать знаки рубы, режим знаков: 0 пробелы, 1 авто, 2
/// знаки).
pub(crate) fn text_justify(v: &str) -> Option<(bool, bool, u8)> {
    Some(match v.to_ascii_lowercase().as_str() {
        "auto" => (false, false, 1),
        "none" => (true, false, 1),
        "inter-word" => (false, false, 0),
        "inter-character" | "distribute" => (false, false, 2),
        "ruby" => (false, true, 1),
        _ => return None,
    })
}

/// `text-transform: none | math-auto | [capitalize | uppercase | lowercase]
/// || full-width || full-size-kana` (css-text-4 §2.1): регистр и добавки.
pub(crate) fn text_transform(v: &str) -> Option<(TextTransform, u8)> {
    let lower = v.to_ascii_lowercase();
    match lower.trim() {
        "none" => return Some((TextTransform::None, 0)),
        "math-auto" => return Some((TextTransform::None, TT_MATH)),
        _ => {}
    }
    let (mut case, mut flags) = (None, 0u8);
    for word in lower.split_ascii_whitespace() {
        let flag = match word {
            "uppercase" | "lowercase" | "capitalize" if case.is_none() => {
                case = Some(match word {
                    "uppercase" => TextTransform::Upper,
                    "lowercase" => TextTransform::Lower,
                    _ => TextTransform::Capitalize,
                });
                continue;
            }
            "full-width" => TT_FULL_WIDTH,
            "full-size-kana" => TT_KANA,
            _ => return None,
        };
        if flags & flag != 0 {
            return None;
        }
        flags |= flag;
    }
    (case.is_some() || flags != 0).then(|| (case.unwrap_or(TextTransform::None), flags))
}

/// `tab-size: <number [0,∞]> | <length [0,∞]>`: множитель ширины пробела или
/// длина.
pub(crate) enum TabSizeValue {
    Spaces(f32),
    Length(Len),
}

pub(crate) fn tab_size(v: &str, vcx: ValueCx<'_>) -> Option<TabSizeValue> {
    if let Some(n) = css_number(v) {
        return (n.is_finite() && n >= 0.0).then_some(TabSizeValue::Spaces(n));
    }
    let length = Len::parse(v, vcx)?;
    let ok = match length {
        Len::Px(n)
        | Len::Em(n)
        | Len::Ch(n)
        | Len::Ex(n)
        | Len::Ic(n)
        | Len::Lh(n)
        | Len::Vh(n)
        | Len::Vw(n) => n.is_finite() && n >= 0.0,
        Len::Calc(_) | Len::EmPx(_, _) | Len::LhPx(_, _) => true,
        _ => false,
    };
    ok.then_some(TabSizeValue::Length(length))
}

pub(super) fn accepts(id: LonghandId, v: &str, vcx: ValueCx<'_>) -> Option<bool> {
    use LonghandId::*;
    Some(match id {
        LetterSpacing | WordSpacing => Len::parse_spacing(v, vcx).is_some(),
        TextIndent => text_indent(v, vcx).is_some(),
        TextAlignAll => align(v, false).is_some(),
        TextAlignLast => align(v, true).is_some(),
        TextJustify => text_justify(v).is_some(),
        TextTransform => text_transform(v).is_some(),
        TabSize => tab_size(v, vcx).is_some(),
        _ => return None,
    })
}

impl Computed {
    pub(super) fn apply_spacing(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) -> bool {
        use LonghandId::*;
        match id {
            LetterSpacing => {
                self.letter_spacing = Len::parse_spacing(v, vcx).or(self.letter_spacing)
            }
            WordSpacing => self.word_spacing = Len::parse_spacing(v, vcx).or(self.word_spacing),
            TextIndent => {
                if let Some((length, each, hanging)) = text_indent(v, vcx) {
                    self.text_indent = Some(length);
                    self.text_indent_each_line = Some(each);
                    self.text_indent_hanging = Some(hanging);
                }
            }
            TextAlignAll => match align(v, false) {
                Some(None) => {
                    self.text_align = None;
                    self.text_align_match_parent |= 1;
                }
                Some(Some(a)) => {
                    self.text_align = a;
                    self.text_align_match_parent &= !1;
                }
                None => {}
            },
            TextAlignLast => match align(v, true) {
                Some(None) => {
                    self.text_align_last = None;
                    self.text_align_match_parent |= 2;
                }
                Some(Some(a)) => {
                    self.text_align_last = Some(a);
                    self.text_align_match_parent &= !2;
                }
                None => {}
            },
            TextJustify => {
                if let Some((none, ruby, chars)) = text_justify(v) {
                    self.no_justify = Some(none);
                    self.ruby_justify = Some(ruby);
                    self.justify_chars = Some(chars);
                }
            }
            TextTransform => {
                if let Some((case, flags)) = text_transform(v) {
                    self.text_transform = Some(case);
                    self.text_transform_flags = flags;
                }
            }
            TabSize => match tab_size(v, vcx) {
                Some(TabSizeValue::Spaces(n)) => {
                    (self.tab_size, self.tab_size_len) = (Some(n), None)
                }
                Some(TabSizeValue::Length(l)) => {
                    (self.tab_size, self.tab_size_len) = (None, Some(l))
                }
                None => {}
            },
            _ => return false,
        }
        true
    }

    pub(super) fn inherit_spacing(&mut self, id: LonghandId) -> bool {
        use LonghandId::*;
        match id {
            LetterSpacing => self.letter_spacing = None,
            WordSpacing => self.word_spacing = None,
            TextIndent => {
                self.text_indent = None;
                self.text_indent_each_line = None;
                self.text_indent_hanging = None;
            }
            TextAlignAll => {
                self.text_align = None;
                self.text_align_match_parent &= !1;
            }
            TextAlignLast => {
                self.text_align_last = None;
                self.text_align_match_parent &= !2;
            }
            TextJustify => {
                self.no_justify = None;
                self.ruby_justify = None;
                self.justify_chars = None;
            }
            TextTransform => {
                self.text_transform = None;
                self.text_transform_flags = 0;
            }
            TabSize => (self.tab_size, self.tab_size_len) = (None, None),
            _ => return false,
        }
        true
    }
}
