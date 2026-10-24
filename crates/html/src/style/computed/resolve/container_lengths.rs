//! Единицы контейнера в составных computed values: до наследования потомками и paint.
use crate::document::state::ValueCx;
use crate::style::computed::{Computed, DecorLen, ViewportCx};
use crate::style::values::value::Len;

impl Computed {
    pub(super) fn resolve_container_other_lengths(&mut self, vp: ViewportCx, vcx: ValueCx<'_>) {
        let fold = |l: &mut Len| *l = l.fold_container(vp, vcx);
        for l in [
            &mut self.lanes_tolerance,
            &mut self.svg_x,
            &mut self.svg_y,
            &mut self.offset_distance,
            &mut self.vertical_shift_len,
            &mut self.clip_round_len,
            &mut self.column_rule_width,
            &mut self.row_rule_width,
            &mut self.shape_margin,
        ]
        .into_iter()
        .flatten()
        {
            fold(l);
        }
        for l in self.fit_arg.iter_mut().flatten() {
            fold(l);
        }
        for (x, y) in [&mut self.translate, &mut self.mask_size, &mut self.mask_pos]
            .into_iter()
            .flatten()
        {
            fold(x);
            fold(y);
        }
        for (x, y) in self.clip_polygon.iter_mut().flatten() {
            fold(x);
            fold(y);
        }
        for (x, y, _, _) in self.mask_pos_list.iter_mut().flatten() {
            fold(x);
            fold(y);
        }
        for list in [&mut self.clip_len, &mut self.clip_edges]
            .into_iter()
            .flatten()
        {
            for l in list.iter_mut().flatten() {
                fold(l);
            }
        }
        for list in [&mut self.clip_inset, &mut self.clip_xywh]
            .into_iter()
            .flatten()
        {
            list.iter_mut().for_each(fold);
        }
        if let Some((_, list)) = &mut self.object_view_box {
            list.iter_mut().for_each(fold);
        }
        for list in [&mut self.column_rule_widths, &mut self.row_rule_widths]
            .into_iter()
            .flatten()
        {
            list.lead
                .iter_mut()
                .chain(&mut list.auto)
                .chain(&mut list.tail)
                .for_each(fold);
        }
        if let Some(DecorLen::Raw(l)) = &mut self.td_thickness {
            fold(l);
        }
    }
}
