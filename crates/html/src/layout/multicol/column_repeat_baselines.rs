//! Базовые линии повторяемых полос таблицы (шапка и подвал) в колонках: полоса предлагает
//! только те строки содержимого, что видны в её отрисованной маске, с теми же индексами
//! копий, маской и началом, что в `ColumnStack::prepaint`/`paint`.

use super::*;
use crate::layout::fragment::types::{Frag, StackChild};
use crate::tolerance::COORD_EPS;
use gpui::{App, LayoutMeasurement, Window, px};

#[derive(Default)]
pub(super) struct RepeatedMeasurements {
    head: Option<f32>,
    foot: Option<(f32, f32)>,
    heads: Vec<LayoutMeasurement>,
    feet: Vec<LayoutMeasurement>,
}

impl RepeatedMeasurements {
    pub(super) async fn new_async(
        child: &mut StackChild,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self {
        let Some(repeat) = child.repeat.as_mut() else {
            return Self::default();
        };
        let mut heads = Vec::with_capacity(repeat.head_els.len());
        let mut feet = Vec::with_capacity(repeat.foot_els.len());
        for el in &mut repeat.head_els {
            heads.push(
                el.snapshot_layout_measurement_async(window, cx, stack)
                    .await,
            );
        }
        for el in &mut repeat.foot_els {
            feet.push(
                el.snapshot_layout_measurement_async(window, cx, stack)
                    .await,
            );
        }
        Self {
            head: repeat.head.map(|band| band.0),
            foot: repeat.foot,
            heads,
            feet,
        }
    }

    pub(super) async fn measure_async(
        &mut self,
        fragment: &Frag,
        available: gpui::Size<AvailableSpace>,
        y: f32,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (Baselines, Baselines) {
        let mut head = Baselines::default();
        let mut foot = Baselines::default();
        // Match the copy indices, mask and origin in ColumnStack::prepaint/paint.
        if fragment.head > COORD_EPS
            && fragment.copy > 0
            && let (Some(source), Some(measure)) =
                (self.head, self.heads.get_mut(fragment.copy - 1))
        {
            let (_, first, last) = measure
                .measure_slice_async(available, px(source), px(fragment.head), window, cx, stack)
                .await;
            head.include(
                first.map(f32::from),
                last.map(f32::from),
                source,
                fragment.head,
                y - fragment.head,
            );
        }
        if fragment.foot > COORD_EPS
            && let (Some((source, height)), Some(measure)) =
                (self.foot, self.feet.get_mut(fragment.copy))
        {
            let (_, first, last) = measure
                .measure_slice_async(available, px(source), px(height), window, cx, stack)
                .await;
            foot.include(
                first.map(f32::from),
                last.map(f32::from),
                source,
                height,
                y + fragment.h + fragment.foot - height,
            );
        }
        (head, foot)
    }
}
