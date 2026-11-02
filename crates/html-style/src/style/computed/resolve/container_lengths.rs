//! Единицы контейнера в составных computed values: до наследования потомками и paint.
use crate::style::computed::{Computed, DecorLen, ViewportCx};
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    pub(super) fn resolve_container_other_lengths(&mut self, vp: ViewportCx, vcx: ValueCx<'_>) {
        // Тот же предикат, что у Sum::fold_container: без cq-слагаемых
        // проход не меняет значений и сохраняет общие группы.
        if !self.has_other_container_lengths(vcx) {
            return;
        }
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

    fn has_other_container_lengths(&self, vcx: ValueCx<'_>) -> bool {
        let needs = |l: &Len| l.needs_container_context(vcx);
        [
            self.reset_grid.lanes_tolerance,
            self.reset_svg_math.svg_x,
            self.reset_svg_math.svg_y,
            self.reset_transforms.offset_distance,
            self.reset_text_box.vertical_shift_len,
            self.reset_mask.clip_round_len,
            self.reset_fragmentation.column_rule_width,
            self.reset_fragmentation.row_rule_width,
            self.reset_shapes.shape_margin,
        ]
        .into_iter()
        .flatten()
        .any(|l| needs(&l))
            || self.reset_context.fit_arg.iter().flatten().any(needs)
            || [
                self.reset_transforms.translate.as_ref(),
                self.reset_mask.mask_size.as_ref(),
                self.reset_mask.mask_pos.as_ref(),
            ]
            .into_iter()
            .flatten()
            .chain(self.reset_mask.clip_polygon.iter().flatten())
            .any(|(x, y)| needs(x) || needs(y))
            || self
                .reset_mask
                .mask_pos_list
                .iter()
                .flatten()
                .any(|(x, y, _, _)| needs(x) || needs(y))
            || [&self.reset_mask.clip_len, &self.reset_mask.clip_edges]
                .into_iter()
                .flatten()
                .any(|list| list.iter().flatten().any(needs))
            || [&self.reset_mask.clip_inset, &self.reset_mask.clip_xywh]
                .into_iter()
                .flatten()
                .any(|list| list.iter().any(needs))
            || self
                .reset_shapes
                .object_view_box
                .as_ref()
                .is_some_and(|(_, list)| list.iter().any(needs))
            || [
                &self.reset_fragmentation.column_rule_widths,
                &self.reset_fragmentation.row_rule_widths,
            ]
            .into_iter()
            .flatten()
            .any(|list| {
                list.lead
                    .iter()
                    .chain(&list.auto)
                    .chain(&list.tail)
                    .any(needs)
            })
            || matches!(self.reset_text_box.td_thickness, Some(DecorLen::Raw(l)) if needs(&l))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_container_operands_keep_shared_groups() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let mut base = Computed::default();
        for (key, value) in [
            ("shape-margin", "4px"),
            ("translate", "5px 6px"),
            ("column-rule-width", "2px"),
        ] {
            base.apply_one(key, value, doc.values());
        }
        let mut value = base.clone();
        value.resolve_container_other_lengths(value.viewport_cx((800.0, 600.0)), doc.values());
        assert!(base.shares_storage(&value));
    }

    #[test]
    fn container_operands_detach_and_resolve_without_changing_source() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let mut base = Computed::default();
        base.apply_one("shape-margin", "10cqw", doc.values());
        let mut value = base.clone();
        value.resolve_container_other_lengths(value.viewport_cx((800.0, 600.0)), doc.values());
        assert_eq!(value.reset_shapes.shape_margin, Some(Len::Px(80.0)));
        assert!(
            base.reset_shapes
                .shape_margin
                .unwrap()
                .needs_container_context(doc.values())
        );
    }
}
