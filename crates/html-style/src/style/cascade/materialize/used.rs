//! Used-слой меняет только свойства, отличающиеся от исходной computed-стадии.
use crate::style::computed::*;
macro_rules! overlay {
    ($fn:ident, $record:ident, $($field:ident),* $(,)?) => {
        pub(super) fn $fn(base: &mut StyleGroup<$record>, used: &StyleGroup<$record>, computed: &StyleGroup<$record>) {
            if used.ptr_eq(computed) { return; }
            let $record { $($field),* } = &**used;
            $(if *$field != computed.$field { base.$field = $field.clone(); })*
        }
    };
}
pub(super) fn lay_used(base: &mut Computed, used: &Computed, computed: &Computed) {
    if used.decor_box != computed.decor_box {
        base.decor_box = used.decor_box;
    }
    if used.block_tag != computed.block_tag {
        base.block_tag = used.block_tag;
    }
    reset_text_box(
        &mut base.reset_text_box,
        &used.reset_text_box,
        &computed.reset_text_box,
    );
    reset_cascade(
        &mut base.reset_cascade,
        &used.reset_cascade,
        &computed.reset_cascade,
    );
    reset_box_layout(
        &mut base.reset_box_layout,
        &used.reset_box_layout,
        &computed.reset_box_layout,
    );
    reset_flex(&mut base.reset_flex, &used.reset_flex, &computed.reset_flex);
    reset_border(
        &mut base.reset_border,
        &used.reset_border,
        &computed.reset_border,
    );
    reset_grid(&mut base.reset_grid, &used.reset_grid, &computed.reset_grid);
    reset_context(
        &mut base.reset_context,
        &used.reset_context,
        &computed.reset_context,
    );
    reset_generated(
        &mut base.reset_generated,
        &used.reset_generated,
        &computed.reset_generated,
    );
    reset_svg_math(
        &mut base.reset_svg_math,
        &used.reset_svg_math,
        &computed.reset_svg_math,
    );
    reset_positioned(
        &mut base.reset_positioned,
        &used.reset_positioned,
        &computed.reset_positioned,
    );
    reset_effects(
        &mut base.reset_effects,
        &used.reset_effects,
        &computed.reset_effects,
    );
    reset_background(
        &mut base.reset_background,
        &used.reset_background,
        &computed.reset_background,
    );
    reset_mask(&mut base.reset_mask, &used.reset_mask, &computed.reset_mask);
    reset_shapes(
        &mut base.reset_shapes,
        &used.reset_shapes,
        &computed.reset_shapes,
    );
    reset_fragmentation(
        &mut base.reset_fragmentation,
        &used.reset_fragmentation,
        &computed.reset_fragmentation,
    );
    reset_transforms(
        &mut base.reset_transforms,
        &used.reset_transforms,
        &computed.reset_transforms,
    );
    if !std::rc::Rc::ptr_eq(&used.inh.0, &computed.inh.0) {
        inherited_ui(&mut base.inh.ui, &used.inh.ui, &computed.inh.ui);
        inherited_font(&mut base.inh.font, &used.inh.font, &computed.inh.font);
        inherited_text_flow(
            &mut base.inh.text_flow,
            &used.inh.text_flow,
            &computed.inh.text_flow,
        );
        inherited_table_list(
            &mut base.inh.table_list,
            &used.inh.table_list,
            &computed.inh.table_list,
        );
        inherited_text_ink(
            &mut base.inh.text_ink,
            &used.inh.text_ink,
            &computed.inh.text_ink,
        );
        inherited_writing(
            &mut base.inh.writing,
            &used.inh.writing,
            &computed.inh.writing,
        );
    }
}
mod groups_0;
use groups_0::*;

mod groups_1;
use groups_1::*;

mod groups_2;
use groups_2::*;

mod groups_3;
use groups_3::*;

mod groups_4;
use groups_4::*;

mod groups_5;
use groups_5::*;
