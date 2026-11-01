//! Наследуемое оформление и руби: `text-shadow` (css-text-decor-4 §5),
//! `text-underline-offset`, `text-underline-position` (§2.4-2.5),
//! `text-decoration-skip-ink`, `-skip-spaces` (§2.7-2.8),
//! `text-emphasis-position` (§3.2), `ruby-position`, `ruby-align`,
//! `ruby-overhang`, `ruby-merge` (css-ruby-1 §4).

use crate::style::computed::{
    Computed, DecorLen, RubyAlign, RubyOverhang, UPOS_FROM_FONT, UPOS_LEFT, UPOS_RIGHT, UPOS_UNDER,
    box_shadow_valid, has_font_units, parse_decor_length, parse_shadows,
};
use crate::style::css::LonghandId;
use crate::values::ValueCx;

/// `text-shadow: none | <shadow>#` (без `inset`).
pub fn text_shadow_ok(v: &str, vcx: ValueCx<'_>) -> bool {
    v.eq_ignore_ascii_case("none")
        || (box_shadow_valid(v, vcx)
            && !v
                .split_whitespace()
                .any(|w| w.eq_ignore_ascii_case("inset")))
}

/// `text-underline-offset: auto | <length-percentage>`.
pub fn underline_offset(v: &str, vcx: ValueCx<'_>) -> Option<DecorLen> {
    let t = v.to_ascii_lowercase();
    if t == "auto" {
        return Some(DecorLen::Auto);
    }
    parse_decor_length(&t, vcx)
}

/// `text-underline-position: auto | from-font | [under || [left | right]]`.
pub fn underline_position(v: &str) -> Option<u8> {
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
pub fn skip_ink(v: &str) -> Option<u8> {
    match v.to_ascii_lowercase().as_str() {
        "none" => Some(0),
        "auto" => Some(1),
        "all" => Some(2),
        _ => None,
    }
}

/// `text-decoration-skip-spaces: none | all | [start || end]`: разряды 1
/// начало, 2 конец, 4 все.
pub fn skip_spaces(v: &str) -> Option<u8> {
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
pub fn emphasis_position(v: &str) -> Option<Option<(bool, bool)>> {
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
pub fn ruby_position(v: &str) -> Option<bool> {
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

pub fn ruby_align(v: &str) -> Option<RubyAlign> {
    Some(match v.to_ascii_lowercase().as_str() {
        "start" => RubyAlign::Start,
        "center" => RubyAlign::Center,
        "space-between" => RubyAlign::SpaceBetween,
        "space-around" => RubyAlign::SpaceAround,
        _ => return None,
    })
}

pub fn ruby_overhang(v: &str) -> Option<RubyOverhang> {
    Some(match v.to_ascii_lowercase().as_str() {
        "auto" => RubyOverhang::Auto,
        "none" => RubyOverhang::None,
        "spaces" => RubyOverhang::Spaces,
        _ => return None,
    })
}

/// `ruby-merge: separate | merge | auto`: 0, 1, 2.
pub fn ruby_merge(v: &str) -> Option<u8> {
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

mod apply;
