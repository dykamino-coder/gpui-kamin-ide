//! Обрезка холста маской корневого элемента.
use crate::style::computed::{Computed, ResetMask, StyleGroup};
pub(super) fn style(st: &Computed) -> Computed {
    Computed {
        reset_mask: StyleGroup::new(ResetMask {
            clip_polygon: st.reset_mask.clip_polygon.clone(),
            clip_shape: st.reset_mask.clip_shape.clone(),
            clip_inset: st.reset_mask.clip_inset.clone(),
            clip_xywh: st.reset_mask.clip_xywh.clone(),
            ..Default::default()
        }),
        ..Default::default()
    }
}
