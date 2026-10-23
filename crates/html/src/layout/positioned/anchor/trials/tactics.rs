//! Тактики действуют на входы sizing и потомков, до построения кандидата.

use super::super::area::{Tracks, physical_area};
use super::super::parse::{FLIP_BLOCK, FLIP_INLINE, FLIP_START};
use crate::document::state::ValueCx;
use crate::style::computed::{Align, Computed, Sides};
use crate::style::css::LonghandId as L;
use crate::style::values::position_area::{AreaAxis, AreaKw, AreaSide, PositionArea};
use crate::style::values::value::{AnchorFn, AnchorSide, Len, anchor_get, anchor_store};

pub(in crate::layout::positioned::anchor) fn apply(
    c: &mut Computed,
    cb: &Computed,
    bits: u8,
    vcx: ValueCx<'_>,
) {
    for bit in [FLIP_BLOCK, FLIP_INLINE, FLIP_START] {
        if bits & bit == 0 {
            continue;
        }
        let mut map = [0, 1, 2, 3];
        if bit == FLIP_START {
            let b = cb.wm().block_start().index();
            let i = cb.wm().inline_start().index();
            map.swap(b, i);
            map.swap((b + 2) % 4, (i + 2) % 4);
        } else {
            let y = if bit == FLIP_BLOCK {
                !cb.wm().vertical()
            } else {
                cb.wm().vertical()
            };
            if y {
                map.swap(0, 2);
            } else {
                map.swap(1, 3);
            }
        }
        let transpose = map[0] % 2 == 1;
        if let Some(area) = c.position_area {
            let (x, y) = physical_area(area, cb, c);
            let mut tracks = [x, y];
            for (axis, t) in [(0, x), (1, y)] {
                let start = if axis == 0 { 3 } else { 0 };
                let new_axis = usize::from(map[start] % 2 == 0);
                tracks[new_axis] = if map[start] == 3 || map[start] == 0 {
                    t
                } else {
                    Tracks {
                        lo: 3 - t.hi,
                        hi: 3 - t.lo,
                    }
                };
            }
            c.position_area = Some(PositionArea(
                area_side(tracks[0], AreaAxis::X),
                area_side(tracks[1], AreaAxis::Y),
            ));
        }
        c.inset = sides(c.inset, map, |l, old| transform(l, old, map, cb, c, vcx));
        c.margin = sides(c.margin, map, |l, _| l);
        let permute = |input: [bool; 4]| {
            let mut out = [false; 4];
            for k in 0..4 {
                out[map[k]] = input[k];
            }
            out
        };
        for ids in [
            [L::MarginTop, L::MarginRight, L::MarginBottom, L::MarginLeft],
            [L::Top, L::Right, L::Bottom, L::Left],
        ] {
            let flags = permute(ids.map(|id| c.inherit_longhands.contains(id)));
            for (id, on) in ids.into_iter().zip(flags) {
                c.inherit_longhands.set(id, on);
            }
        }
        if transpose {
            super::sizing::transpose(c, vcx);
            std::mem::swap(&mut c.align_self, &mut c.justify_self);
            std::mem::swap(&mut c.align_self_safe, &mut c.justify_self_safe);
            std::mem::swap(&mut c.align_self_normal, &mut c.justify_self_normal);
            std::mem::swap(&mut c.align_self_own_axis, &mut c.justify_self_own_axis);
            std::mem::swap(&mut c.align_self_last, &mut c.justify_self_last);
            c.align_self_decl = Some(c.align_self);
        } else {
            let y = map[0] == 2;
            let align = if y != cb.wm().vertical() {
                &mut c.align_self
            } else {
                &mut c.justify_self
            };
            *align = match *align {
                Some(Align::Start) => Some(Align::End),
                Some(Align::End) => Some(Align::Start),
                other => other,
            };
            c.align_self_decl = Some(c.align_self);
        }
    }
}

fn sides(s: Sides, map: [usize; 4], f: impl Fn(Option<Len>, usize) -> Option<Len>) -> Sides {
    let mut out = [None; 4];
    for (k, l) in [s.top, s.right, s.bottom, s.left].into_iter().enumerate() {
        out[map[k]] = f(l, k);
    }
    Sides {
        top: out[0],
        right: out[1],
        bottom: out[2],
        left: out[3],
    }
}

fn area_side(t: Tracks, axis: AreaAxis) -> AreaSide {
    let kw = match (t.lo, t.hi) {
        (0, 1) => AreaKw::Start,
        (1, 2) => AreaKw::Center,
        (2, 3) => AreaKw::End,
        (0, 2) => AreaKw::SpanStart,
        (1, 3) => AreaKw::SpanEnd,
        _ => AreaKw::SpanAll,
    };
    AreaSide {
        kw,
        axis,
        logical: false,
        self_wm: false,
    }
}

fn transform(
    l: Option<Len>,
    old: usize,
    map: [usize; 4],
    cb: &Computed,
    own: &Computed,
    vcx: ValueCx<'_>,
) -> Option<Len> {
    let Some(Len::Anchor(id)) = l else { return l };
    let mut f = (*anchor_get(id, vcx)?).clone();
    if f.size.is_some() {
        return l;
    }
    let flips = |c: &Computed| super::super::plan::flipped(c);
    let side = |s| {
        let start = if old.is_multiple_of(2) { 0 } else { 3 };
        let logical = |c: &Computed, end: bool| {
            let flip = if start == 0 { flips(c).1 } else { flips(c).0 };
            (start + if flip != end { 2 } else { 0 }) % 4
        };
        let k = match s {
            AnchorSide::Top => 0,
            AnchorSide::Right => 1,
            AnchorSide::Bottom => 2,
            AnchorSide::Left => 3,
            AnchorSide::Start => logical(cb, false),
            AnchorSide::End => logical(cb, true),
            AnchorSide::SelfStart => logical(own, false),
            AnchorSide::SelfEnd => logical(own, true),
            AnchorSide::Pct(p) => {
                return AnchorSide::Pct(
                    if map[0].is_multiple_of(2) && (map[start] == 1 || map[start] == 2) {
                        1. - p
                    } else {
                        p
                    },
                );
            }
            other => return other,
        };
        [
            AnchorSide::Top,
            AnchorSide::Right,
            AnchorSide::Bottom,
            AnchorSide::Left,
        ][map[k]]
    };
    fn change(f: &mut AnchorFn, side: &impl Fn(AnchorSide) -> AnchorSide) {
        f.side = side(f.side);
        for a in &mut f.alts {
            change(a, side);
        }
    }
    change(&mut f, &side);
    f.fallback = transform(f.fallback, old, map, cb, own, vcx);
    Some(Len::Anchor(anchor_store(f, vcx)))
}
