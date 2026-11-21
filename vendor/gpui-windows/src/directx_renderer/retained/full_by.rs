//! KaminIDE patch: why a frame is drawn whole, and the count of every plan
//! in the `[gpu]` line (`plan keep=… full=… within=… full_by=…`).

use super::super::DirectXRenderer;
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
    pub(in crate::directx_renderer) fn count_plan(&self, plan: &Plan) {
        use gpui::batch_cost::FramePlan;
        let window = self.width as f64 * self.height as f64;
        let counted = match plan {
            Plan::Keep => FramePlan::Keep,
            Plan::Full => FramePlan::Full(self.retained.full_by.name()),
            Plan::Within(r) => {
                FramePlan::Within(r.iter().map(|r| r.width as f64 * r.height as f64).sum())
            }
        };
        self.gpu_timer.instrumentation.batches.plan(counted, window);
    }
}

#[cfg(test)]
mod tests {
    use super::FullBy;
    use gpui::SceneDamage;

    #[test]
    fn an_unplaced_external_change_comes_first_then_the_target() {
        let rect = SceneDamage::Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        let unknown = SceneDamage::Unknown;
        assert_eq!(FullBy::of(unknown, true, rect, rect), FullBy::External);
        assert_eq!(FullBy::of(rect, true, rect, unknown), FullBy::Target);
        assert_eq!(FullBy::of(rect, false, rect, unknown), FullBy::Unknown);
        assert_eq!(
            FullBy::of(SceneDamage::Unchanged, false, rect, rect),
            FullBy::Earlier
        );
    }
}
