//! Shared traversal of deferred lengths in explicit and implicit grid tracks.

use super::*;
use crate::style::computed::groups::ResetGrid;

impl ResetGrid {
    pub(crate) fn track_sizes(&self) -> impl Iterator<Item = &TrackSize> {
        [
            self.grid_tracks.as_ref(),
            self.grid_rows.as_ref(),
            self.auto_repeat_body_cols.as_ref(),
            self.auto_repeat_body_rows.as_ref(),
        ]
        .into_iter()
        .flatten()
        .flatten()
        .chain(self.grid_auto_cols.iter())
        .chain(self.grid_auto_rows.iter())
        .chain(self.grid_auto_cols_list.iter())
        .chain(self.grid_auto_rows_list.iter())
    }

    pub(crate) fn for_each_track_mut(&mut self, mut f: impl FnMut(&mut TrackSize)) {
        for list in [
            &mut self.grid_tracks,
            &mut self.grid_rows,
            &mut self.auto_repeat_body_cols,
            &mut self.auto_repeat_body_rows,
        ]
        .into_iter()
        .flatten()
        {
            for track in list {
                f(track);
            }
        }
        for track in self
            .grid_auto_cols
            .iter_mut()
            .chain(self.grid_auto_rows.iter_mut())
            .chain(self.grid_auto_cols_list.iter_mut())
            .chain(self.grid_auto_rows_list.iter_mut())
        {
            f(track);
        }
    }
}

impl TrackSize {
    pub(crate) fn lengths_pending(&self, moves: impl Fn(Len) -> bool + Copy) -> bool {
        let one = |t: &Track| match t {
            Track::Font(l) | Track::FitFont(l) => moves(*l),
            _ => false,
        };
        match self {
            Self::Single(t) => one(t),
            Self::MinMax(lo, hi) => one(lo) || one(hi),
            Self::AutoRepeat { tracks, .. } => tracks.iter().any(|t| t.lengths_pending(moves)),
        }
    }

    pub(crate) fn resolve_lengths(&mut self, fix: impl Fn(&mut Option<Len>) + Copy) {
        let one = |t: &mut Track| {
            let (l, fit) = match *t {
                Track::Font(l) => (l, false),
                Track::FitFont(l) => (l, true),
                _ => return,
            };
            let mut next = Some(l);
            fix(&mut next);
            *t = match (next.unwrap_or(l), fit) {
                (Len::Px(v), false) => Track::Px(v),
                (Len::Pct(v), false) => Track::Pct(v),
                (Len::Px(v), true) => Track::FitPx(v),
                (Len::Pct(v), true) => Track::FitPct(v),
                (l, false) => Track::Font(l),
                (l, true) => Track::FitFont(l),
            };
        };
        match self {
            Self::Single(t) => one(t),
            Self::MinMax(lo, hi) => {
                one(lo);
                one(hi);
            }
            Self::AutoRepeat { tracks, .. } => {
                for t in tracks {
                    t.resolve_lengths(fix);
                }
            }
        }
    }
}
