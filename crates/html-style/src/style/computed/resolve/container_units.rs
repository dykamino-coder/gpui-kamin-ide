//! Контекст единиц контейнера: только used content box, переданный оркестратором
//! после завершённой раскладки (css-conditional-5 §container-lengths).

use crate::style::computed::{Computed, ContainerType, Display};
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    /// Кегль и строка разрешаются раньше зависимых от них em/lh.
    pub fn resolve_container_font_lengths(&mut self, vcx: ValueCx<'_>) {
        let vp = self.viewport_cx(vcx.viewport);
        if let Some(value) = self
            .inh
            .font
            .font_size
            .filter(|l| l.needs_container_context(vcx))
        {
            self.inh.font.font_size = Some(value.fold_container(vp, vcx));
        }
        if let Some(value) = self
            .inh
            .font
            .line_height
            .filter(|l| l.needs_container_context(vcx))
        {
            self.inh.font.line_height = Some(value.fold_container(vp, vcx));
        }
    }

    /// Свернуть единицы по завершённой раскладке ближайших подходящих предков.
    pub fn resolve_container_units(&mut self, vcx: ValueCx<'_>) {
        let vp = self.viewport_cx(vcx.viewport);
        let fix = |length: &mut Option<Len>| {
            if let Some(value) = *length {
                *length = Some(value.fold_container(vp, vcx));
            }
        };
        self.resolve_unit_lengths(fix, vcx);
        self.resolve_container_other_lengths(vp, vcx);
        for length in {
            let group_0 = &mut *self.reset_fragmentation;
            [
                &mut group_0.column_width,
                &mut group_0.column_height,
                &mut group_0.column_gap,
            ]
        } {
            fix(length);
        }
        // Без cq-слагаемых общую группу наследуемых копировать не нужно.
        if [
            self.inh.text_flow.letter_spacing,
            self.inh.text_flow.word_spacing,
            self.inh.text_flow.text_indent,
            self.inh.font.line_height,
            self.inh.text_flow.tab_size_len,
        ]
        .into_iter()
        .flatten()
        .any(|l| l.needs_container_context(vcx))
        {
            let inherited = &mut *self.inh;
            for length in {
                let group_0 = &mut *inherited.text_flow;
                [
                    &mut group_0.letter_spacing,
                    &mut group_0.word_spacing,
                    &mut group_0.text_indent,
                    &mut inherited.font.line_height,
                    &mut group_0.tab_size_len,
                ]
            } {
                fix(length);
            }
        }
        if let Some(outline) = self.reset_border.outline.as_mut() {
            fix(&mut outline.width);
            fix(&mut outline.offset);
        }
        if let Some((row, column)) = self.inh.table_list.border_spacing
            && [row, column]
                .into_iter()
                .flatten()
                .any(|l| l.needs_container_context(vcx))
        {
            let mut spacing = (row, column);
            fix(&mut spacing.0);
            fix(&mut spacing.1);
            self.inh.table_list.border_spacing = Some(spacing);
        }
        self.resolve_decor_inset_lengths(fix, vcx);
    }

    /// Поддерживаемые физические оси. Письмо здесь — контейнера, тогда как
    /// cqi/cqb выбирают логическую ось по письму самого потребителя.
    pub fn query_container_axes(&self) -> (bool, bool) {
        if !self.reset_box_layout.container_type.is_size_query()
            || matches!(
                self.reset_box_layout.display,
                Some(Display::None | Display::Contents)
            )
        {
            return (false, false);
        }
        let block = self.reset_box_layout.container_type == ContainerType::Size;
        if self.wm().vertical() {
            (block && self.contains_width(), self.contains_height())
        } else {
            (self.contains_width(), block && self.contains_height())
        }
    }

    /// Ближайший контейнер выбирается отдельно для каждой оси. Неизвестный
    /// layout input подходящего контейнера НЕ разрешает пропустить его:
    /// промежуточный стиль будет пересчитан до paint после его раскладки.
    pub fn container_box_for_children(&self) -> (Option<f32>, Option<f32>) {
        let (width, height) = self.query_container_axes();
        (
            if width {
                self.reset_context.cq_size.0
            } else {
                self.reset_context.cq_box.0
            },
            if height {
                self.reset_context.cq_size.1
            } else {
                self.reset_context.cq_box.1
            },
        )
    }
}
