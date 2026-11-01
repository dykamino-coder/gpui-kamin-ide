//! Начальные CSS-группы создаются документом и разделяются его каскадом через COW.
use super::Computed;

impl Default for Computed {
    fn default() -> Self {
        Self {
            decor_box: 0,
            block_tag: false,
            plain_block_box: false,
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
        }
    }
}

#[cfg(feature = "work-counters")]
impl Computed {
    pub(crate) fn track_initial(&mut self, owner: &crate::work::style_storage::Owner) {
        self.clone_audit.track(owner);
        self.reset_text_box.track(owner);
        self.reset_cascade.track(owner);
        self.reset_box_layout.track(owner);
        self.reset_flex.track(owner);
        self.reset_border.track(owner);
        self.reset_grid.track(owner);
        self.reset_context.track(owner);
        self.reset_generated.track(owner);
        self.reset_svg_math.track(owner);
        self.reset_positioned.track(owner);
        self.reset_effects.track(owner);
        self.reset_background.track(owner);
        self.reset_mask.track(owner);
        self.reset_shapes.track(owner);
        self.reset_fragmentation.track(owner);
        self.reset_transforms.track(owner);
        let inherited = std::rc::Rc::get_mut(&mut self.inh.0).expect("fresh inherited header");
        inherited.inherited_clone_audit.track(owner);
        inherited.ui.track(owner);
        inherited.font.track(owner);
        inherited.text_flow.track(owner);
        inherited.table_list.track(owner);
        inherited.text_ink.track(owner);
        inherited.writing.track(owner);
    }
}
