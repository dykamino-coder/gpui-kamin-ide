//! Preserve both border-radius axes until their length and percentage bases are known.

use super::{Computed, Corners};
use crate::style::values::value::Len;

impl Computed {
    pub(super) fn apply_radius_shorthand(&mut self, raw: &str) {
        let Some((horizontal, vertical)) = axes(raw) else {
            return;
        };
        let Some(h) = shorthand(horizontal) else {
            return;
        };
        let v = match vertical {
            Some(raw) => match shorthand(raw) {
                Some(v) => v,
                None => return,
            },
            None => h,
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
        let tokens = crate::paint::background::split_top(raw);
        let (x, y) = match tokens.as_slice() {
            [x] => (length(x), length(x)),
            [x, y] => (length(x), length(y)),
            _ => return,
        };
        if x.is_none() || y.is_none() {
            return;
        }
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
        let resolve = |radius: &mut Option<Len>| {
            fix(radius);
            if let Some(Len::Px(value)) = radius {
                *value = value.max(0.0);
            }
        };
        for corner in [
            &mut self.radius.tl,
            &mut self.radius.tr,
            &mut self.radius.br,
            &mut self.radius.bl,
        ] {
            resolve(corner);
        }
        if let Some(corners) = self.radius_ell.as_mut() {
            for (x, y) in corners.iter_mut().flatten() {
                let mut rx = Some(*x);
                let mut ry = Some(*y);
                resolve(&mut rx);
                resolve(&mut ry);
                *x = rx.unwrap_or(Len::Px(0.0));
                *y = ry.unwrap_or(Len::Px(0.0));
            }
        }
    }
}

// CSS Backgrounds 3 §5.1: only a top-level slash separates the radius axes.
// Division and whitespace inside a math function belong to one length token.
fn axes(raw: &str) -> Option<(&str, Option<&str>)> {
    let mut depth = 0usize;
    let mut slash = None;
    for (i, ch) in raw.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            '/' if depth == 0 && slash.replace(i).is_some() => {
                return None;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    Some(match slash {
        Some(i) => (&raw[..i], Some(&raw[i + 1..])),
        None => (raw, None),
    })
}

fn length(raw: &str) -> Option<Len> {
    match super::size_range::parse(raw)? {
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => None,
        length => Some(length),
    }
}

fn shorthand(raw: &str) -> Option<Corners> {
    let tokens = crate::paint::background::split_top(raw);
    let values: Vec<Len> = tokens.into_iter().map(length).collect::<Option<_>>()?;
    let [tl, tr, br, bl] = match values.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    };
    Some(Corners {
        tl: Some(tl),
        tr: Some(tr),
        br: Some(br),
        bl: Some(bl),
    })
}
