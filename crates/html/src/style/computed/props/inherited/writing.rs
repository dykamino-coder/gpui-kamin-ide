//! Письмо и математика: `direction`, `writing-mode` (css-writing-modes-4
//! §2.1, §3.1, устаревшие слова SVG 1.1 §3.2), `text-orientation` (§5.1),
//! `text-combine-upright` (§9.1), `math-depth`, `math-shift`, `math-style`
//! (MathML Core §3.1-3.2).

use crate::style::computed::Computed;
use crate::style::computed::math_style::{Depth, depth};
use crate::style::computed::writing_mode::{TextOrientation, WritingModeKw};
use crate::style::css::LonghandId;

/// `direction: ltr | rtl`: `true` — справа налево.
pub(crate) fn direction(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "ltr" => Some(false),
        "rtl" => Some(true),
        _ => None,
    }
}

/// `writing-mode: horizontal-tb | vertical-rl | vertical-lr | sideways-rl |
/// sideways-lr` и устаревшие значения SVG (css-writing-modes-4 §3.2):
/// `lr`, `lr-tb`, `rl`, `rl-tb` — `horizontal-tb`; `tb`, `tb-rl` —
/// `vertical-rl`.
pub(crate) fn writing_mode(v: &str) -> Option<WritingModeKw> {
    let lower = v.to_ascii_lowercase();
    let keyword = match lower.as_str() {
        "horizontal-tb" | "lr" | "lr-tb" | "rl" | "rl-tb" => "horizontal-tb",
        "vertical-rl" | "tb" | "tb-rl" => "vertical-rl",
        k @ ("vertical-lr" | "sideways-rl" | "sideways-lr") => k,
        _ => return None,
    };
    Some(WritingModeKw::parse(keyword))
}

/// `text-orientation: mixed | upright | sideways` (`sideways-right` —
/// устаревший синоним `sideways`, `use-glyph-orientation` — `mixed`,
/// §5.1.1).
pub(crate) fn text_orientation(v: &str) -> Option<TextOrientation> {
    let lower = v.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "mixed" | "upright" | "sideways" | "sideways-right" | "use-glyph-orientation"
    )
    .then(|| TextOrientation::parse(&lower))
}

/// `text-combine-upright: none | all | digits <integer [2,4]>?`: число
/// знаков (`0` — все), `None` — `none`.
pub(crate) fn combine_upright(v: &str) -> Option<Option<u8>> {
    let lower = v.to_ascii_lowercase();
    let words: Vec<_> = lower.split_whitespace().collect();
    match words.as_slice() {
        ["none"] => Some(None),
        ["all"] => Some(Some(0)),
        ["digits"] => Some(Some(2)),
        ["digits", n] => n
            .parse::<u8>()
            .ok()
            .filter(|n| (2..=4).contains(n))
            .map(Some),
        _ => None,
    }
}

/// `math-style` и `math-shift`: `normal | compact`; `true` — `compact`.
pub(crate) fn compact(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "normal" => Some(false),
        "compact" => Some(true),
        _ => None,
    }
}

/// `math-depth: auto-add | add(<integer>) | <integer>`.
pub(crate) fn math_depth(v: &str) -> Option<Depth> {
    depth(&v.to_ascii_lowercase())
}

pub(super) fn accepts(id: LonghandId, v: &str) -> bool {
    use LonghandId::*;
    match id {
        Direction => direction(v).is_some(),
        WritingMode => writing_mode(v).is_some(),
        TextOrientation => text_orientation(v).is_some(),
        TextCombineUpright => combine_upright(v).is_some(),
        MathShift | MathStyle => compact(v).is_some(),
        MathDepth => math_depth(v).is_some(),
        _ => false,
    }
}

impl Computed {
    pub(super) fn apply_writing(&mut self, id: LonghandId, v: &str) {
        use LonghandId::*;
        match id {
            Direction => self.rtl = direction(v).or(self.rtl),
            WritingMode => self.writing_mode = writing_mode(v).or(self.writing_mode),
            TextOrientation => {
                self.text_orientation = text_orientation(v).or(self.text_orientation)
            }
            TextCombineUpright => {
                if let Some(n) = combine_upright(v) {
                    self.combine_upright = Some(n);
                }
            }
            MathShift => self.math_shift = compact(v).or(self.math_shift),
            MathStyle => self.math_compact = compact(v).or(self.math_compact),
            MathDepth => self.math_depth = math_depth(v).or(self.math_depth),
            _ => {}
        }
    }

    pub(super) fn inherit_writing(&mut self, id: LonghandId) {
        use LonghandId::*;
        match id {
            Direction => self.rtl = None,
            WritingMode => self.writing_mode = None,
            TextOrientation => self.text_orientation = None,
            TextCombineUpright => self.combine_upright = None,
            MathShift => self.math_shift = None,
            MathStyle => self.math_compact = None,
            MathDepth => self.math_depth = None,
            _ => {}
        }
    }
}
