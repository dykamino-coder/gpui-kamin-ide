//! Repeated table bands nominate only the content lines visible in their painted masks.

use super::*;

#[derive(Default)]
pub(super) struct RepeatedMeasurements {
    head: Option<f32>,
    foot: Option<(f32, f32)>,
    heads: Vec<LayoutMeasurement>,
    feet: Vec<LayoutMeasurement>,
}

impl RepeatedMeasurements {
    pub(super) fn new(child: &mut StackChild, window: &mut Window, cx: &mut App) -> Self {
        let Some(repeat) = child.repeat.as_mut() else {
            return Self::default();
        };
        Self {
            head: repeat.head.map(|band| band.0),
            foot: repeat.foot,
            heads: repeat
                .head_els
                .iter_mut()
                .map(|el| el.snapshot_layout_measurement(window, cx))
                .collect(),
            feet: repeat
                .foot_els
                .iter_mut()
                .map(|el| el.snapshot_layout_measurement(window, cx))
                .collect(),
        }
    }

    pub(super) fn measure(
        &mut self,
        fragment: &Frag,
        available: gpui::Size<AvailableSpace>,
        y: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> (Baselines, Baselines) {
        let mut head = Baselines::default();
        let mut foot = Baselines::default();
        // Match the copy indices, mask and origin in ColumnStack::prepaint/paint.
        if fragment.head > 0.01 && fragment.copy > 0 {
            if let (Some(source), Some(measure)) =
                (self.head, self.heads.get_mut(fragment.copy - 1))
            {
                let (_, first, last) =
                    measure.measure_slice(available, px(source), px(fragment.head), window, cx);
                head.include(
                    first.map(f32::from),
                    last.map(f32::from),
                    source,
                    fragment.head,
                    y - fragment.head,
                );
            }
        }
        if fragment.foot > 0.01 {
            if let (Some((source, height)), Some(measure)) =
                (self.foot, self.feet.get_mut(fragment.copy))
            {
                let (_, first, last) =
                    measure.measure_slice(available, px(source), px(height), window, cx);
                foot.include(
                    first.map(f32::from),
                    last.map(f32::from),
                    source,
                    height,
                    y + fragment.h + fragment.foot - height,
                );
            }
        }
        (head, foot)
    }
}
