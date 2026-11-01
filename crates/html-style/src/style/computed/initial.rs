//! Начальные CSS-группы создаются документом и разделяются его каскадом через COW.
#[cfg(feature = "work-counters")]
use super::Computed;

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
