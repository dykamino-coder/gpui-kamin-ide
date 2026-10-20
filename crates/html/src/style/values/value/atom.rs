//! Numeric length atoms shared by Len storage and arena-free calc evaluation.

use super::{
    Len, Sum, calc_store, css_number, root_font_px, root_font_unit, root_font_view, root_line_px,
};
use crate::document::state::ValueCx;

pub(super) struct Atom {
    nature: u8,
    value: f32,
}

impl Atom {
    pub(super) fn parse(s: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let split = s
            .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%')
            .len();
        let (number, unit) = s.split_at(split);
        let value = css_number(number)?;
        let atom = |nature, value| Self { nature, value };
        for (name, scale) in [
            ("px", 1.0),
            ("pt", 96.0 / 72.0),
            ("in", 96.0),
            ("cm", 96.0 / 2.54),
            ("mm", 96.0 / 25.4),
            ("pc", 16.0),
            ("q", 96.0 / 101.6),
        ] {
            if unit.eq_ignore_ascii_case(name) {
                return Some(atom(0, value * scale));
            }
        }
        for (name, nature) in [
            ("%", 1),
            ("em", 2),
            ("ch", 3),
            ("ex", 4),
            ("ic", 5),
            ("cap", 6),
            ("lh", 7),
        ] {
            if unit.eq_ignore_ascii_case(name) {
                return Some(atom(
                    nature,
                    if nature == 1 { value / 100.0 } else { value },
                ));
            }
        }
        if let Some(nature) = super::units::parse_unit(unit) {
            return Some(atom(nature, value / 100.0));
        }
        if unit.eq_ignore_ascii_case("rem") {
            return Some(match root_font_view(vcx) {
                Some((high, k)) => atom(if high { 8 } else { 9 }, value * k),
                None => atom(0, value * root_font_px(vcx)),
            });
        }
        if unit.eq_ignore_ascii_case("rlh") {
            return Some(atom(0, value * root_line_px(vcx)));
        }
        for name in ["rcap", "rch", "rex", "ric"] {
            if unit.eq_ignore_ascii_case(name) {
                return Some(atom(0, value * root_font_unit(&name[1..], vcx)));
            }
        }
        None
    }

    pub(super) fn sum(&self) -> Sum {
        Sum::of_nature(self.nature, self.value)
    }

    pub(super) fn length(&self, vcx: ValueCx<'_>) -> Len {
        let v = self.value;
        match self.nature {
            0 => Len::Px(v),
            1 => Len::Pct(v),
            2 => Len::Em(v),
            3 => Len::Ch(v),
            4 => Len::Ex(v),
            5 => Len::Ic(v),
            7 => Len::Lh(v),
            8 => Len::Vh(v),
            9 => Len::Vw(v),
            _ => Len::Calc(calc_store(self.sum(), vcx)),
        }
    }
}
