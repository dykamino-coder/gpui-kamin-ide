//! KaminIDE patch: why a frame is drawn whole, and the count of every plan
//! in the `[gpu]` line (`plan keep=… full=… within=… full_by=…`).

use super::super::DirectXRenderer;
use super::super::canvas::Drawn;
use super::Plan;
use gpui::SceneDamage;

/// Why a frame is drawn whole: the `[gpu]` line counts these (`full_by=`).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(in crate::directx_renderer) enum FullBy {
    /// A sampled external texture whose change cannot be placed.
    External,
    /// Size, background, backdrop or atlas epoch differ from the last frame.
    Target,
    /// The scene was not compared with the frame drawn last.
    #[default]
    Unknown,
    /// The buffer still lacks an earlier frame that was drawn whole.
    Earlier,
    /// The damage covers the whole target.
    Region,
    /// Partial redraw is off, or the buffer index is unknown.
    Disabled,
}

impl FullBy {
    /// The reason a frame would be drawn whole before the buffer is chosen:
    /// `retarget` when the target inputs differ from the last frame's,
    /// `damage` the scene's own, `frame` after comparison.
    pub(in crate::directx_renderer) fn of(
        external: SceneDamage,
        retarget: bool,
        damage: SceneDamage,
        frame: SceneDamage,
    ) -> Self {
        if external == SceneDamage::Unknown {
            FullBy::External
        } else if retarget {
            FullBy::Target
        } else if damage == SceneDamage::Unknown || frame == SceneDamage::Unknown {
            FullBy::Unknown
        } else {
            FullBy::Earlier
        }
    }

    fn name(self) -> &'static str {
        match self {
            FullBy::External => "external",
            FullBy::Target => "target",
            FullBy::Unknown => "unknown",
            FullBy::Earlier => "earlier",
            FullBy::Region => "region",
            FullBy::Disabled => "disabled",
        }
    }
}

impl DirectXRenderer {
    /// Count the plan in the `[gpu]` line: kept, whole (and why) or the area
    /// of the rectangle drawn.
    pub(in crate::directx_renderer) fn count_plan(&mut self, plan: &Plan, shift_miss: Option<&'static str>, shift_stats: Option<gpui::ShiftStats>) {
        use gpui::batch_cost::FramePlan;
        let window = self.width as f64 * self.height as f64;
        let area = |r: &super::Region| r.width as u64 * r.height as u64;
        let (w, h) = (self.width, self.height);
        let (counted, drawn) = match plan {
            Plan::Keep => (FramePlan::Keep, Drawn { plan: "keep", ..Default::default() }),
            Plan::Full => {
                let reason = self.retained.full_by.name();
                (FramePlan::Full(reason), Drawn { plan: "full", full_by: Some(reason), rects: vec![[0, 0, w, h]], ..Default::default() })
            }
            Plan::Within(r) => (
                FramePlan::Within(area(r) as f64),
                Drawn { plan: "within", rects: vec![[r.x, r.y, r.width, r.height]], ..Default::default() },
            ),
            Plan::Shift { delta, copy, redraw } => {
                let px: u64 = redraw.iter().map(area).sum();
                let rects = redraw.iter().map(|r| [r.x, r.y, r.width, r.height]).collect();
                // A frame compared in place (`FrameShift::in_place`) only redraws.
                let drawn = match *delta {
                    (0, 0) => Drawn { plan: "within", rects, ..Default::default() },
                    delta => Drawn {
                        plan: "shift",
                        delta: Some(delta),
                        moved_px: copy[2] as u64 * copy[3] as u64,
                        rects,
                        ..Default::default()
                    },
                };
                (FramePlan::Within(px as f64), drawn)
            }
        };
        self.retained.drawn = super::super::canvas::Drawn { shift_miss, shift_stats, ..drawn };
        self.gpu_timer.instrumentation.batches.plan(counted, window);
    }
}

#[cfg(test)]
mod tests {
    use super::FullBy;
    use gpui::SceneDamage;

    #[test]
    fn an_unplaced_external_change_comes_first_then_the_target() {
        let rect = SceneDamage::Rect { x: 0, y: 0, width: 1, height: 1 };
        let unknown = SceneDamage::Unknown;
        assert_eq!(FullBy::of(unknown, true, rect, rect), FullBy::External);
        assert_eq!(FullBy::of(rect, true, rect, unknown), FullBy::Target);
        assert_eq!(FullBy::of(rect, false, rect, unknown), FullBy::Unknown);
        assert_eq!(FullBy::of(SceneDamage::Unchanged, false, rect, rect), FullBy::Earlier);
    }
}
