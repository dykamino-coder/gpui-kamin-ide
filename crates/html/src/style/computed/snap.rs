//! CSS Scroll Snap 1: non-inherited snap type, alignment, snapport and snap area.

use super::{Computed, Sides, split_outside_parens};
use crate::document::state::ValueCx;
use crate::style::values::value::Len;

#[derive(Clone, Copy, Debug, Default)]
pub struct SnapStyle {
    /// 0 none, 1 x, 2 y, 3 block, 4 inline, 5 both.
    pub axis: u8,
    /// 1 proximity, 2 mandatory.
    pub strictness: u8,
    /// Block and inline: 0 none, 1 start, 2 end, 3 center.
    pub align: [u8; 2],
    pub padding: Sides,
    pub margin: Sides,
    pub inherit: u16,
}

impl Computed {
    pub(crate) fn apply_snap(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        let bit = match key {
            "scroll-snap-type" => 1,
            "scroll-snap-align" => 2,
            "scroll-padding" => 0x3c,
            "scroll-margin" => 0x3c0,
            "scroll-padding-top" => 4,
            "scroll-padding-right" => 8,
            "scroll-padding-bottom" => 16,
            "scroll-padding-left" => 32,
            "scroll-margin-top" => 64,
            "scroll-margin-right" => 128,
            "scroll-margin-bottom" => 256,
            "scroll-margin-left" => 512,
            _ => 0,
        };
        if bit != 0 && v == "inherit" {
            self.snap.inherit |= bit;
            return;
        }
        match key {
            "scroll-snap-type" => {
                let parts: Vec<_> = v.split_whitespace().collect();
                if parts == ["none"] {
                    self.accept_declaration();
                    self.snap.inherit &= !bit;
                    self.snap.axis = 0;
                    self.snap.strictness = 0;
                    return;
                }
                let axis = match parts.first().copied() {
                    Some("x") => 1,
                    Some("y") => 2,
                    Some("block") => 3,
                    Some("inline") => 4,
                    Some("both") => 5,
                    _ => return,
                };
                let strictness = match parts.get(1).copied() {
                    None | Some("proximity") => 1,
                    Some("mandatory") => 2,
                    _ => return,
                };
                if parts.len() <= 2 {
                    self.accept_declaration();
                    self.snap.inherit &= !bit;
                    self.snap.axis = axis;
                    self.snap.strictness = strictness;
                }
            }
            "scroll-snap-align" => {
                let parse = |word| match word {
                    "none" => Some(0),
                    "start" => Some(1),
                    "end" => Some(2),
                    "center" => Some(3),
                    _ => None,
                };
                let parts: Vec<_> = v.split_whitespace().collect();
                if (1..=2).contains(&parts.len())
                    && let (Some(block), Some(inline)) =
                        (parse(parts[0]), parse(*parts.get(1).unwrap_or(&parts[0])))
                {
                    self.accept_declaration();
                    self.snap.inherit &= !bit;
                    self.snap.align = [block, inline];
                }
            }
            _ if key.starts_with("scroll-padding") || key.starts_with("scroll-margin") => {
                let padding = key.starts_with("scroll-padding");
                let prefix = if padding {
                    "scroll-padding"
                } else {
                    "scroll-margin"
                };
                let sides = if padding {
                    &mut self.snap.padding
                } else {
                    &mut self.snap.margin
                };
                let parts = split_outside_parens(v);
                let lengths: Option<Vec<_>> = parts
                    .iter()
                    .map(|s| {
                        if padding && s == "auto" {
                            Some(Len::Auto)
                        } else {
                            Len::parse_mixed(s, vcx).filter(|l| match *l {
                                Len::Px(n)
                                | Len::Em(n)
                                | Len::Ch(n)
                                | Len::Ex(n)
                                | Len::Ic(n)
                                | Len::Lh(n)
                                | Len::Vw(n)
                                | Len::Vh(n) => !padding || n >= 0.0,
                                Len::Pct(n) => padding && n >= 0.0,
                                Len::Calc(i) => {
                                    padding
                                        || !crate::style::values::value::calc_get(i, vcx)
                                            .has_percentage
                                }
                                Len::EmPx(_, _) | Len::LhPx(_, _) => true,
                                _ => false,
                            })
                        }
                    })
                    .collect();
                let Some(lengths) = lengths else { return };
                match key.strip_prefix(prefix).unwrap_or("") {
                    "" if (1..=4).contains(&lengths.len()) => *sides = Sides::shorthand(v, vcx),
                    "-top" if lengths.len() == 1 => sides.top = Some(lengths[0]),
                    "-right" if lengths.len() == 1 => sides.right = Some(lengths[0]),
                    "-bottom" if lengths.len() == 1 => sides.bottom = Some(lengths[0]),
                    "-left" if lengths.len() == 1 => sides.left = Some(lengths[0]),
                    _ => *hit = false,
                }
                if *hit {
                    self.accept_declaration();
                    self.snap.inherit &= !bit;
                }
            }
            _ => *hit = false,
        }
    }
}

pub(crate) fn inherit(parent: &Computed, c: &mut Computed) {
    let bits = c.snap.inherit;
    if bits & 1 != 0 {
        c.snap.axis = parent.snap.axis;
        c.snap.strictness = parent.snap.strictness;
    }
    if bits & 2 != 0 {
        c.snap.align = parent.snap.align;
    }
    for (own, inherited, first) in [
        (&mut c.snap.padding, parent.snap.padding, 2),
        (&mut c.snap.margin, parent.snap.margin, 6),
    ] {
        for (i, (out, value)) in [
            (&mut own.top, inherited.top),
            (&mut own.right, inherited.right),
            (&mut own.bottom, inherited.bottom),
            (&mut own.left, inherited.left),
        ]
        .into_iter()
        .enumerate()
        {
            if bits & (1 << (first + i)) != 0 {
                *out = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Snap cascade regression checks, including invalid declarations and CSS-wide resets.
    use crate::style::{cascade::inherit::inherit, computed::Computed, values::value::Len};

    #[test]
    fn snap_is_local_and_explicit_inheritance_copies_only_requested_properties() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_one("scroll-snap-type", "both mandatory", vcx);
        parent.apply_one("scroll-snap-align", "end center", vcx);
        parent.apply_one("scroll-padding", "10px 20px", vcx);
        parent.apply_one("scroll-margin", "5px", vcx);
        let mut own = Computed::default();
        assert_eq!(inherit(&parent, &own, vcx).snap.axis, 0);
        own.apply_one("scroll-snap-type", "inherit", vcx);
        own.apply_one("scroll-padding-left", "inherit", vcx);
        let child = inherit(&parent, &own, vcx);
        assert_eq!(child.snap.axis, 5);
        assert_eq!(child.snap.strictness, 2);
        assert_eq!(child.snap.align, [0; 2]);
        assert_eq!(child.snap.padding.left, Some(Len::Px(20.0)));
        assert_eq!(child.snap.padding.top, None);
        assert_eq!(child.snap.margin.top, None);
        own.apply_one("scroll-snap-type", "initial", vcx);
        assert_eq!(inherit(&parent, &own, vcx).snap.axis, 0);
    }

    #[test]
    fn snap_invalid_values_preserve_valid_declarations() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("scroll-snap-align", "start end", vcx);
        c.apply_one("scroll-snap-align", "start invalid", vcx);
        assert_eq!(c.snap.align, [1, 2]);
        c.apply_one("scroll-padding", "10px", vcx);
        c.apply_one("scroll-padding", "-1px", vcx);
        assert_eq!(c.snap.padding.top, Some(Len::Px(10.0)));
        c.apply_one("scroll-margin", "-5px", vcx);
        c.apply_one("scroll-margin", "50%", vcx);
        assert_eq!(c.snap.margin.top, Some(Len::Px(-5.0)));
    }
}
