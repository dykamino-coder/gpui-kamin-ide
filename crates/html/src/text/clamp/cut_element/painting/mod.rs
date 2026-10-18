//! Apply a pure clamp plan to the existing frame-to-frame clamp state.

mod automatic;
mod boxes;
mod counted;
mod fit;
mod geometry;
mod paragraph;
#[cfg(test)]
mod tests;

use super::ClampCut;
use super::{ClampEntry, clamp_cut, clamp_para, take_para_rows};
use crate::text::clamp::line_state::{CLAMP_BOXES, CLAMP_EXTENT, CLAMP_NEXT};
use crate::text::clamp::{CLAMP_CUTS, CLAMP_PARA};
pub(crate) use boxes::ClampBox;
pub(super) use boxes::{chain_end, chain_extra, chain_sizes};
use geometry::ClampGeometry;
use gpui::{App, Bounds, Pixels, Window};

impl ClampCut {
    // Keep the extracted GPUI Element paint signature intact.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_impl(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let all_entries = std::mem::take(&mut *self.lines.borrow_mut());
        let para_rows = take_para_rows(self.key);
        let prev_para = clamp_para(self.key);
        let plan = ClampGeometry {
            limit: self.limit,
            max_h: self.max_h,
            trim_end: self.trim_end,
        }
        .compute(
            all_entries,
            &para_rows,
            f32::from(bounds.origin.y),
            prev_para,
        );
        let (para, next, extent) = (plan.para, plan.next, plan.extent);
        // The boxes fragmented at the point end there too.
        let mut boxes_moved = false;
        CLAMP_BOXES.with(|m| {
            let mut m = m.borrow_mut();
            let mine = &plan.nodes;
            for n in mine {
                let now = plan.sizes.iter().find(|(k, _)| k == n).map(|(_, h)| *h);
                let before = m.get(n).copied();
                let differs = match (before, now) {
                    (Some(a), Some(b)) => (a - b).abs() > 0.5,
                    (a, b) => a.is_some() != b.is_some(),
                };
                if differs {
                    boxes_moved = true;
                    match now {
                        Some(h) => m.insert(*n, h),
                        None => m.remove(n),
                    };
                }
            }
        });
        if boxes_moved {
            window.request_animation_frame();
        }
        if prev_para != para {
            CLAMP_PARA.with(|m| {
                let mut m = m.borrow_mut();
                match para {
                    Some(v) => {
                        m.insert(self.key, v);
                    }
                    None => {
                        m.remove(&self.key);
                    }
                }
            });
            window.request_animation_frame();
        }
        if crate::text::clamp::line_state::clamp_next(self.key) != next {
            CLAMP_NEXT.with(|m| {
                let mut m = m.borrow_mut();
                match next {
                    Some(v) => m.insert(self.key, v),
                    None => m.remove(&self.key),
                };
            });
            window.request_animation_frame();
        }
        let prev_extent = crate::text::clamp::clamp_extent(self.key);
        let moved = match (prev_extent, extent) {
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            (a, b) => a.is_some() != b.is_some(),
        };
        if moved {
            CLAMP_EXTENT.with(|m| {
                let mut m = m.borrow_mut();
                match extent {
                    Some(v) => m.insert(self.key, v),
                    None => m.remove(&self.key),
                };
            });
            window.request_animation_frame();
        }
        // Infinity records a completed measurement with no cut; an absent
        // registry entry means no measurement has been applied yet.
        let v = plan.cut;
        let prev = clamp_cut(self.key);
        // Preserve the existing tolerance for clamp-induced margin movement.
        let changed = match prev {
            Some(a) if a.is_finite() && v.is_finite() => (a - v).abs() > 4.0,
            Some(a) => a.is_finite() != v.is_finite(),
            None => true,
        };
        if changed {
            CLAMP_CUTS.with(|m| {
                m.borrow_mut().insert(self.key, v);
            });
            window.request_animation_frame();
        }
    }
}
