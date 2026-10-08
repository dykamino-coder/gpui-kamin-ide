//! Preserve both border-radius axes until their length and percentage bases are known.

use super::{Computed, radius_shorthand};
use crate::value::Len;

impl Computed {
    pub(super) fn apply_radius_shorthand(&mut self, raw: &str) {
        let (h, v) = match raw.split_once('/') {
            Some((h, v)) => (radius_shorthand(h.trim()), radius_shorthand(v.trim())),
            None => {
                let h = radius_shorthand(raw);
                (h, h)
            }
        };
        self.radius = h;
        // CSS Backgrounds 3 §5.1: the slash supplies independent vertical radii.
        // Preserve the units; percentage bases are only available at paint time.
        let pair = |a, b| match (a, b) {
            (Some(a), Some(b)) if a != b => Some((a, b)),
            _ => None,
        };
        let ell = [
            pair(h.tl, v.tl),
            pair(h.tr, v.tr),
            pair(h.br, v.br),
            pair(h.bl, v.bl),
        ];
        self.radius_ell = ell.iter().any(Option::is_some).then_some(ell);
    }

    pub(super) fn apply_radius_corner(&mut self, key: &str, raw: &str) {
        let mut tokens = raw.split_whitespace();
        let x = tokens.next().and_then(Len::parse);
        let y = tokens.next().and_then(Len::parse).or(x);
        let slot = match key {
            "border-top-left-radius" => 0,
            "border-top-right-radius" => 1,
            "border-bottom-right-radius" => 2,
            _ => 3,
        };
        match slot {
            0 => self.radius.tl = x,
            1 => self.radius.tr = x,
            2 => self.radius.br = x,
            _ => self.radius.bl = x,
        }
        let mut ell = self.radius_ell.unwrap_or([None; 4]);
        ell[slot] = match (x, y) {
            (Some(x), Some(y)) if x != y => Some((x, y)),
            _ => None,
        };
        self.radius_ell = ell.iter().any(Option::is_some).then_some(ell);
    }

    pub(crate) fn resolve_radius_lengths(&mut self, fix: impl Fn(&mut Option<Len>)) {
        for corner in [
            &mut self.radius.tl,
            &mut self.radius.tr,
            &mut self.radius.br,
            &mut self.radius.bl,
        ] {
            fix(corner);
        }
        if let Some(corners) = self.radius_ell.as_mut() {
            for (x, y) in corners.iter_mut().flatten() {
                let mut rx = Some(*x);
                let mut ry = Some(*y);
                fix(&mut rx);
                fix(&mut ry);
                *x = rx.unwrap_or(Len::Px(0.0));
                *y = ry.unwrap_or(Len::Px(0.0));
            }
        }
    }
}
