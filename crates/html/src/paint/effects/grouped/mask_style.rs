//! Transfer computed mask properties to the grouped paint wrapper.
use super::*;

pub(in crate::paint::effects) fn apply_mask_composite(
    wrapper: &mut crate::paint::effects::grouped_element::Grouped,
    c: &Computed,
) {
    wrapper.mask_composite = c.reset_mask.mask_composite.clone().unwrap_or_default();
}

pub(in crate::paint::effects) fn apply_mask_style(
    wrapper: &mut crate::paint::effects::grouped_element::Grouped,
    c: &Computed,
) {
    wrapper.mask_size = c.reset_mask.mask_size.clone();
    wrapper.mask_fit = c.reset_mask.mask_fit.unwrap_or(0);
    wrapper.mask_no_repeat = c.reset_mask.mask_no_repeat.unwrap_or((false, false));
    wrapper.mask_repeat_list = c.reset_mask.mask_repeat_list.clone().unwrap_or_default();
    wrapper.mask_repeat_modes = c.reset_mask.mask_repeat_modes.clone().unwrap_or_default();
    wrapper.mask_luminance = c.reset_mask.mask_luminance == Some(true);
    wrapper.mask_alpha_mode = c.reset_mask.mask_alpha_mode == Some(true);
    wrapper.mask_orient_none = c.inh.ui.image_orient_none == Some(true);
    wrapper.mask_pos = c.reset_mask.mask_pos.clone();
    wrapper.mask_pos_far = c.reset_mask.mask_pos_far;
    wrapper.mask_pos_list = c.reset_mask.mask_pos_list.clone().unwrap_or_default();
}
