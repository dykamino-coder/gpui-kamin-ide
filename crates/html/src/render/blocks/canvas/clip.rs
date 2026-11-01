//! Обрезка холста маской корневого элемента.
use crate::style::computed::{Computed, ResetMask, StyleGroup};
pub(super) fn style(st: &Computed) -> Computed {
    Computed {
        reset_mask: StyleGroup::new(ResetMask {
            clip_polygon: st.clip_polygon.clone(),
            clip_shape: st.clip_shape.clone(),
            clip_inset: st.clip_inset,
            clip_xywh: st.clip_xywh,
            ..Default::default()
        }),
        ..Default::default()
    }
}
