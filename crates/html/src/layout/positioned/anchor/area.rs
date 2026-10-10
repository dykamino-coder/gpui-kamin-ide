//! `position-area`: дорожки сетки области в физических осях и выравнивание по умолчанию.

use super::parse::{AreaAxis, AreaKw, AreaSide, PositionArea};
use super::plan::flipped;
use crate::style::computed::Computed;

/// Линии сетки 3×3 по одной физической оси: 0 — начало содержащего блока,
/// 1 — начало якоря, 2 — конец якоря, 3 — конец содержащего блока; уже в
/// физическом порядке (`lo` левее/выше).
#[derive(Clone, Copy, Debug)]
pub(super) struct Tracks {
    pub(super) lo: u8,
    pub(super) hi: u8,
}

fn tracks_of(kw: AreaKw, flip: bool) -> Tracks {
    let (lo, hi) = match kw {
        AreaKw::Start => (0, 1),
        AreaKw::Center => (1, 2),
        AreaKw::End => (2, 3),
        AreaKw::SpanStart => (0, 2),
        AreaKw::SpanEnd => (1, 3),
        AreaKw::SpanAll => (0, 3),
    };
    if flip {
        Tracks {
            lo: 3 - hi,
            hi: 3 - lo,
        }
    } else {
        Tracks { lo, hi }
    }
}

/// Область в физических осях `(x, y)` — как `PositionArea::ToPhysical` у
/// Blink: неясное слово берёт ось, противоположную соседу; оба неясных —
/// первое блочная ось, второе строчная (письмо коробки, если слова `self-*`).
pub(super) fn physical_area(area: PositionArea, cb: &Computed, own: &Computed) -> (Tracks, Tracks) {
    let wm_vertical = |s: AreaSide| {
        if s.self_wm {
            own.vertical == Some(true)
        } else {
            cb.vertical == Some(true)
        }
    };
    let axis_x = |s: AreaSide| -> Option<bool> {
        match s.axis {
            AreaAxis::X => Some(true),
            AreaAxis::Y => Some(false),
            AreaAxis::Block => Some(wm_vertical(s)),
            AreaAxis::Inline => Some(!wm_vertical(s)),
            AreaAxis::Any => None,
        }
    };
    let (a_x, b_x) = match (axis_x(area.0), axis_x(area.1)) {
        (Some(a), Some(b)) if a != b => (a, b),
        (Some(a), _) => (a, !a),
        (None, Some(b)) => (!b, b),
        (None, None) => {
            let v = if area.0.self_wm || area.1.self_wm {
                own.vertical == Some(true)
            } else {
                cb.vertical == Some(true)
            };
            (v, !v)
        }
    };
    // Логическое слово переворачивается письмом своей оси (как `start`/`end`
    // в `anchor()`); физическое — никогда.
    let flip_of = |s: AreaSide, x: bool| -> bool {
        if !s.logical {
            return false;
        }
        let f = flipped(if s.self_wm { own } else { cb });
        if x { f.0 } else { f.1 }
    };
    let ta = tracks_of(area.0.kw, flip_of(area.0, a_x));
    let tb = tracks_of(area.1.kw, flip_of(area.1, b_x));
    if a_x { (ta, tb) } else { (tb, ta) }
}

/// Выравнивание коробки в клетке по одной оси.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Al {
    Start,
    Center,
    End,
    AnchorCenter,
    Stretch,
}

/// Умолчание `normal` по области (§position-area-alignment; Blink
/// `AlignJustifySelfFromPhysical`): только центр → `center`; все три →
/// `anchor-center`; иначе — к неназванной дорожке.
pub(super) fn default_al(t: Tracks) -> Al {
    match (t.lo, t.hi) {
        (0, 3) => Al::AnchorCenter,
        (1, 2) => Al::Center,
        (0, _) => Al::End,
        _ => Al::Start,
    }
}
