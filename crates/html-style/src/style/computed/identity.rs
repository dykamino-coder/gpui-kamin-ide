//! Тождество удерживаемых групп: ключ кэша не смешивает float-значения по PartialEq.
use super::Computed;

impl Computed {
    pub(crate) fn shares_storage(&self, other: &Self) -> bool {
        self.block_tag == other.block_tag
            && std::rc::Rc::ptr_eq(&self.inh.0, &other.inh.0)
            && self.reset_text_box.ptr_eq(&other.reset_text_box)
            && self.reset_cascade.ptr_eq(&other.reset_cascade)
            && self.reset_box_layout.ptr_eq(&other.reset_box_layout)
            && self.reset_flex.ptr_eq(&other.reset_flex)
            && self.reset_border.ptr_eq(&other.reset_border)
            && self.reset_grid.ptr_eq(&other.reset_grid)
            && self.reset_context.ptr_eq(&other.reset_context)
            && self.reset_generated.ptr_eq(&other.reset_generated)
            && self.reset_svg_math.ptr_eq(&other.reset_svg_math)
            && self.reset_positioned.ptr_eq(&other.reset_positioned)
            && self.reset_effects.ptr_eq(&other.reset_effects)
            && self.reset_background.ptr_eq(&other.reset_background)
            && self.reset_mask.ptr_eq(&other.reset_mask)
            && self.reset_shapes.ptr_eq(&other.reset_shapes)
            && self.reset_fragmentation.ptr_eq(&other.reset_fragmentation)
            && self.reset_transforms.ptr_eq(&other.reset_transforms)
    }
}
