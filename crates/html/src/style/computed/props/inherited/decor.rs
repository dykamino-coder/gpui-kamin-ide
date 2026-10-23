//! Наследуемое оформление и руби: `text-shadow` (css-text-decor-4 §5),
//! `text-underline-offset`, `text-underline-position` (§2.4-2.5),
//! `text-decoration-skip-ink`, `-skip-spaces` (§2.7-2.8),
//! `text-emphasis-position` (§3.2), `ruby-position`, `ruby-align`,
//! `ruby-overhang`, `ruby-merge` (css-ruby-1 §4).

use crate::document::state::ValueCx;
use crate::style::computed::{
    Computed, DecorLen, RubyAlign, RubyOverhang, UPOS_FROM_FONT, UPOS_LEFT, UPOS_RIGHT, UPOS_UNDER,
    box_shadow_valid, has_font_units, parse_decor_length, parse_shadows,
};
use crate::style::css::LonghandId;

/// `text-shadow: none | <shadow>#` (без `inset`).
pub(crate) fn text_shadow_ok(v: &str, vcx: ValueCx<'_>) -> bool {
    v.eq_ignore_ascii_case("none")
        || (box_shadow_valid(v, vcx)
            && !v
                .split_whitespace()
                .any(|w| w.eq_ignore_ascii_case("inset")))
}

/// `text-underline-offset: auto | <length-percentage>`.
pub(crate) fn underline_offset(v: &str, vcx: ValueCx<'_>) -> Option<DecorLen> {
    let t = v.to_ascii_lowercase();
    if t == "auto" {
        return Some(DecorLen::Auto);
    }
    parse_decor_length(&t, vcx)
}

/// `text-underline-position: auto | from-font | [under || [left | right]]`.
pub(crate) fn underline_position(v: &str) -> Option<u8> {
    let t = v.to_ascii_lowercase();
    let words: Vec<&str> = t.split_whitespace().collect();
    match words.as_slice() {
        ["auto"] => return Some(0),
        ["from-font"] => return Some(UPOS_FROM_FONT),
        ws if ws.is_empty() || ws.len() > 2 => return None,
        _ => {}
    }
    let mut bits = 0u8;
    for w in words {
        let b = match w {
            "under" => UPOS_UNDER,
            "left" => UPOS_LEFT,
            "right" => UPOS_RIGHT,
            _ => return None,
        };
        let side = UPOS_LEFT | UPOS_RIGHT;
        if bits & b != 0 || (b & side != 0 && bits & side != 0) {
            return None;
        }
        bits |= b;
    }
    Some(bits)
}

/// `text-decoration-skip-ink: auto | none | all`: 1, 0, 2.
pub(crate) fn skip_ink(v: &str) -> Option<u8> {
    match v.to_ascii_lowercase().as_str() {
        "none" => Some(0),
        "auto" => Some(1),
        "all" => Some(2),
        _ => None,
    }
}

/// `text-decoration-skip-spaces: none | all | [start || end]`: разряды 1
/// начало, 2 конец, 4 все.
pub(crate) fn skip_spaces(v: &str) -> Option<u8> {
    let t = v.to_ascii_lowercase();
    match t.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["none"] => Some(0),
        ["all"] => Some(4),
        ["start"] => Some(1),
        ["end"] => Some(2),
        ["start", "end"] | ["end", "start"] => Some(3),
        _ => None,
    }
}

/// `text-emphasis-position: auto | [over | under] && [right | left]?`
/// (`auto` — css-text-decor-4 ED, WPT `text-emphasis-position-valid`):
/// `None` — `auto`, иначе (под строкой, слева).
pub(crate) fn emphasis_position(v: &str) -> Option<Option<(bool, bool)>> {
    let t = v.to_ascii_lowercase();
    let words: Vec<_> = t.split_whitespace().collect();
    if words == ["auto"] {
        return Some(None);
    }
    let (mut vertical, mut side) = (None, None);
    for w in &words {
        match *w {
            "over" | "under" if vertical.is_none() => vertical = Some(*w == "under"),
            "left" | "right" if side.is_none() => side = Some(*w == "left"),
            _ => return None,
        }
    }
    Some(Some((vertical?, side.unwrap_or(false))))
}

/// `ruby-position: [alternate || [over | under]] | inter-character`: `true`
/// — под базой.
pub(crate) fn ruby_position(v: &str) -> Option<bool> {
    let t = v.to_ascii_lowercase();
    let words: Vec<_> = t.split_whitespace().collect();
    if words == ["inter-character"] {
        return Some(false);
    }
    let (mut alternate, mut side) = (false, None);
    for w in &words {
        match *w {
            "alternate" if !alternate => alternate = true,
            "over" | "under" if side.is_none() => side = Some(*w == "under"),
            _ => return None,
        }
    }
    (alternate || side.is_some()).then_some(side == Some(true))
}

pub(crate) fn ruby_align(v: &str) -> Option<RubyAlign> {
    Some(match v.to_ascii_lowercase().as_str() {
        "start" => RubyAlign::Start,
        "center" => RubyAlign::Center,
        "space-between" => RubyAlign::SpaceBetween,
        "space-around" => RubyAlign::SpaceAround,
        _ => return None,
    })
}

pub(crate) fn ruby_overhang(v: &str) -> Option<RubyOverhang> {
    Some(match v.to_ascii_lowercase().as_str() {
        "auto" => RubyOverhang::Auto,
        "none" => RubyOverhang::None,
        "spaces" => RubyOverhang::Spaces,
        _ => return None,
    })
}

/// `ruby-merge: separate | merge | auto`: 0, 1, 2.
pub(crate) fn ruby_merge(v: &str) -> Option<u8> {
    match v.to_ascii_lowercase().as_str() {
        "separate" => Some(0),
        "merge" => Some(1),
        "auto" => Some(2),
        _ => None,
    }
}

pub(super) fn accepts(id: LonghandId, v: &str, vcx: ValueCx<'_>) -> Option<bool> {
    use LonghandId::*;
    Some(match id {
        TextShadow => text_shadow_ok(v, vcx),
        TextUnderlineOffset => underline_offset(v, vcx).is_some(),
        TextUnderlinePosition => underline_position(v).is_some(),
        TextDecorationSkipInk => skip_ink(v).is_some(),
        TextDecorationSkipSpaces => skip_spaces(v).is_some(),
        TextEmphasisPosition => emphasis_position(v).is_some(),
        RubyPosition => ruby_position(v).is_some(),
        RubyAlign => ruby_align(v).is_some(),
        RubyOverhang => ruby_overhang(v).is_some(),
        RubyMerge => ruby_merge(v).is_some(),
        _ => return None,
    })
}

impl Computed {
    pub(super) fn apply_decor(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) -> bool {
        use LonghandId::*;
        match id {
            TextShadow => self.apply_text_shadow(v, vcx),
            TextUnderlineOffset => {
                self.underline_offset = underline_offset(v, vcx).or(self.underline_offset)
            }
            TextUnderlinePosition => {
                self.underline_pos = underline_position(v).or(self.underline_pos)
            }
            TextDecorationSkipInk => self.skip_ink = skip_ink(v).or(self.skip_ink),
            TextDecorationSkipSpaces => self.skip_spaces = skip_spaces(v).or(self.skip_spaces),
            TextEmphasisPosition => match emphasis_position(v) {
                Some(None) => {
                    self.emphasis_position_auto = Some(true);
                    (self.emphasis_under, self.emphasis_left) = (false, false);
                }
                Some(Some((under, left))) => {
                    self.emphasis_position_auto = Some(false);
                    (self.emphasis_under, self.emphasis_left) = (under, left);
                }
                None => {}
            },
            RubyPosition => self.ruby_under = ruby_position(v).or(self.ruby_under),
            RubyAlign => self.ruby_align = ruby_align(v).or(self.ruby_align),
            RubyOverhang => self.ruby_overhang = ruby_overhang(v).or(self.ruby_overhang),
            RubyMerge => self.ruby_merge = ruby_merge(v).or(self.ruby_merge),
            _ => return false,
        }
        true
    }

    /// Тень в единицах шрифта ждёт кегля своего элемента сырой записью
    /// (`resolve_em` разбирает её снова уже в точках): наследуется
    /// вычисленное значение.
    fn apply_text_shadow(&mut self, v: &str, vcx: ValueCx<'_>) {
        if !text_shadow_ok(v, vcx) {
            return;
        }
        self.text_shadow_none = v.eq_ignore_ascii_case("none");
        if has_font_units(v) {
            self.text_shadow_raw = Some(v.to_string());
            self.text_shadow = None;
            self.text_shadow_rest.clear();
            return;
        }
        self.text_shadow_raw = None;
        let mut list = parse_shadows(v, vcx).into_iter();
        self.text_shadow = list.next();
        self.text_shadow_rest = list.collect();
    }

    pub(super) fn inherit_decor(&mut self, id: LonghandId) -> bool {
        use LonghandId::*;
        match id {
            TextShadow => {
                self.text_shadow = None;
                self.text_shadow_raw = None;
                self.text_shadow_none = false;
                self.text_shadow_rest.clear();
            }
            TextUnderlineOffset => self.underline_offset = None,
            TextUnderlinePosition => self.underline_pos = None,
            TextDecorationSkipInk => self.skip_ink = None,
            TextDecorationSkipSpaces => self.skip_spaces = None,
            TextEmphasisPosition => {
                self.emphasis_position_auto = None;
                (self.emphasis_under, self.emphasis_left) = (false, false);
            }
            RubyPosition => self.ruby_under = None,
            RubyAlign => self.ruby_align = None,
            RubyOverhang => self.ruby_overhang = None,
            RubyMerge => self.ruby_merge = None,
            _ => return false,
        }
        true
    }
}
