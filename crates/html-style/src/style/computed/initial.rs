//! Общие начальные CSS-группы потока: COW сохраняет независимость документов.
use super::Computed;

thread_local! {
    static INITIAL: Computed = Computed {
        decor_box: 0, block_tag: false, plain_block_box: false,
        clone_audit: Default::default(),
        reset_text_box: Default::default(),
        reset_cascade: Default::default(),
        reset_box_layout: Default::default(),
        reset_flex: Default::default(),
        reset_border: Default::default(),
        reset_grid: Default::default(),
        reset_context: Default::default(),
        reset_generated: Default::default(),
        reset_svg_math: Default::default(),
        reset_positioned: Default::default(),
        reset_effects: Default::default(),
        reset_background: Default::default(),
        reset_mask: Default::default(),
        reset_shapes: Default::default(),
        reset_fragmentation: Default::default(),
        reset_transforms: Default::default(),
        inh: Default::default(),
    };
}

impl Default for Computed {
    fn default() -> Self {
        INITIAL.with(|initial| Self {
            decor_box: 0,
            block_tag: false,
            plain_block_box: false,
            clone_audit: Default::default(),
            reset_text_box: initial.reset_text_box.clone(),
            reset_cascade: initial.reset_cascade.clone(),
            reset_box_layout: initial.reset_box_layout.clone(),
            reset_flex: initial.reset_flex.clone(),
            reset_border: initial.reset_border.clone(),
            reset_grid: initial.reset_grid.clone(),
            reset_context: initial.reset_context.clone(),
            reset_generated: initial.reset_generated.clone(),
            reset_svg_math: initial.reset_svg_math.clone(),
            reset_positioned: initial.reset_positioned.clone(),
            reset_effects: initial.reset_effects.clone(),
            reset_background: initial.reset_background.clone(),
            reset_mask: initial.reset_mask.clone(),
            reset_shapes: initial.reset_shapes.clone(),
            reset_fragmentation: initial.reset_fragmentation.clone(),
            reset_transforms: initial.reset_transforms.clone(),
            inh: initial.inh.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_groups_are_shared_and_mutations_stay_local() {
        let base = Computed::default();
        let mut changed = Computed::default();
        assert!(base.reset_border.ptr_eq(&changed.reset_border));
        assert!(std::rc::Rc::ptr_eq(&base.inh.0, &changed.inh.0));
        changed.reset_border.border_box = Some(true);
        changed.inh.font.font_weight = Some(700);
        let next = Computed::default();
        assert_eq!(base, next);
        assert!(base.reset_border.ptr_eq(&next.reset_border));
        assert_eq!(next.reset_border.border_box, None);
        assert_eq!(next.inh.font.font_weight, None);
        assert!(!base.reset_border.ptr_eq(&changed.reset_border));
        assert!(base.reset_grid.ptr_eq(&changed.reset_grid));
    }
}
