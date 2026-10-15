//! Directional disclosure symbols, CSS Counter Styles 3 section 6.2.

use crate::style::computed::Computed;

#[derive(Clone, Copy, Default)]
pub struct Context {
    rtl: bool,
    vertical: bool,
    vertical_rl: bool,
}

impl Context {
    pub(crate) fn new(style: &Computed, parent: Self) -> Self {
        Self {
            rtl: style.rtl.unwrap_or(parent.rtl),
            vertical: style.vertical.unwrap_or(parent.vertical),
            vertical_rl: style.vertical_rl.unwrap_or(parent.vertical_rl),
        }
    }

    pub(crate) fn disclosure(self, name: &str) -> Option<&'static str> {
        Some(match (name, self.vertical, self.rtl, self.vertical_rl) {
            ("disclosure-closed", false, false, _) => "▸",
            ("disclosure-closed", false, true, _) => "◂",
            ("disclosure-closed", true, false, _) => "▾",
            ("disclosure-closed", true, true, _) => "▴",
            ("disclosure-open", false, _, _) => "▾",
            ("disclosure-open", true, _, false) => "▸",
            ("disclosure-open", true, _, true) => "◂",
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disclosure_follows_inline_and_block_directions() {
        for (vertical, vertical_rl, rtl, closed, open) in [
            (false, false, false, "▸", "▾"),
            (false, false, true, "◂", "▾"),
            (true, false, false, "▾", "▸"),
            (true, false, true, "▴", "▸"),
            (true, true, false, "▾", "◂"),
            (true, true, true, "▴", "◂"),
        ] {
            let c = Context {
                vertical,
                vertical_rl,
                rtl,
            };
            assert_eq!(c.disclosure("disclosure-closed"), Some(closed));
            assert_eq!(c.disclosure("disclosure-open"), Some(open));
        }
    }
}
