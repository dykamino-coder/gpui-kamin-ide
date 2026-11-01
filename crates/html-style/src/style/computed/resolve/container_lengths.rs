//! Единицы контейнера в составных computed values: до наследования потомками и paint.
use crate::style::computed::{Computed, DecorLen, ViewportCx};
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    pub(super) fn resolve_container_other_lengths(&mut self, vp: ViewportCx, vcx: ValueCx<'_>) {
        let fold = |l: &mut Len| *l = l.fold_container(vp, vcx);
        for l in {
            let group_0 = &mut *self.reset_svg_math;
            let group_1 = &mut *self.reset_fragmentation;
            [
                &mut self.reset_grid.lanes_tolerance,
                &mut group_0.svg_x,
                &mut group_0.svg_y,
                &mut self.reset_transforms.offset_distance,
                &mut self.reset_text_box.vertical_shift_len,
                &mut self.reset_mask.clip_round_len,
                &mut group_1.column_rule_width,
                &mut group_1.row_rule_width,
                &mut self.reset_shapes.shape_margin,
            ]
        }
        .into_iter()
        .flatten()
        {
            fold(l);
        }
        for l in self.reset_context.fit_arg.iter_mut().flatten() {
            fold(l);
        }
        for (x, y) in {
            let group_0 = &mut *self.reset_mask;
            [
                &mut self.reset_transforms.translate,
                &mut group_0.mask_size,
                &mut group_0.mask_pos,
            ]
        }
        .into_iter()
        .flatten()
        {
            fold(x);
            fold(y);
        }
        for (x, y) in self.reset_mask.clip_polygon.iter_mut().flatten() {
            fold(x);
            fold(y);
        }
        for (x, y, _, _) in self.reset_mask.mask_pos_list.iter_mut().flatten() {
            fold(x);
            fold(y);
        }
        for list in {
            let group_0 = &mut *self.reset_mask;
            [&mut group_0.clip_len, &mut group_0.clip_edges]
        }
        .into_iter()
        .flatten()
        {
            for l in list.iter_mut().flatten() {
                fold(l);
            }
        }
        for list in {
            let group_0 = &mut *self.reset_mask;
            [&mut group_0.clip_inset, &mut group_0.clip_xywh]
        }
        .into_iter()
        .flatten()
        {
            list.iter_mut().for_each(fold);
        }
        if let Some((_, list)) = &mut self.reset_shapes.object_view_box {
            list.iter_mut().for_each(fold);
        }
        for list in {
            let group_0 = &mut *self.reset_fragmentation;
            [
                &mut group_0.column_rule_widths,
                &mut group_0.row_rule_widths,
            ]
        }
        .into_iter()
        .flatten()
        {
            list.lead
                .iter_mut()
                .chain(&mut list.auto)
                .chain(&mut list.tail)
                .for_each(fold);
        }
        if let Some(DecorLen::Raw(l)) = &mut self.reset_text_box.td_thickness {
            fold(l);
        }
    }
}
