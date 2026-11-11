//! Разрешение длин `text-decoration-inset` (CSS Text Decoration 4 §4.1):
//! шрифтовые единицы и `lh` — по шрифту и высоте строки элемента после
//! наследования, единицы окна — отдельным проходом при сборке строки, у которой
//! общего прохода разрешения окна нет. Смесь доли и точек остаётся `DecorLen::Mix`.

use super::unit_change::{Change, Target, step};
use crate::style::computed::{Computed, DecorLen};
use crate::style::values::value::{Len, calc_get};
use crate::values::ValueCx;

fn map_length(value: DecorLen, fix: impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) -> DecorLen {
    let DecorLen::Raw(raw) = value else {
        return value;
    };
    let mut length = Some(raw);
    fix(&mut length);
    match length.unwrap_or(raw) {
        Len::Px(v) => DecorLen::Px(v),
        Len::Pct(v) => DecorLen::Pct(v),
        Len::Calc(i) => calc_get(i, vcx)
            .pct_px()
            .map_or(DecorLen::Raw(Len::Calc(i)), |(pct, px)| {
                DecorLen::Mix(pct, px)
            }),
        other => DecorLen::Raw(other),
    }
}

impl Computed {
    pub(super) fn resolve_decor_inset_font(
        &mut self,
        fix: impl Fn(&mut Option<Len>),
        family: &str,
        font: f32,
        vcx: ValueCx<'_>,
    ) {
        if !matches!(self.reset_text_box.td_inset, Some(Some(_))) {
            return;
        }
        let line = match self.inh.font.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k) | Len::Pct(k)) => k * font,
            _ => crate::style::font_metrics::normal_height(family, font, vcx),
        };
        if let Some(Some(pair)) = &mut self.reset_text_box.td_inset {
            for value in pair {
                *value = map_length(
                    *value,
                    |length| {
                        fix(length);
                        if let Some(length) = length {
                            length.resolve_line_height(line, vcx);
                        }
                    },
                    vcx,
                );
            }
        }
    }

    /// Inline collection has no general viewport-resolution pass. Only the
    /// decoration's outstanding viewport terms are resolved there.
    /// Отступы линий оформления, которые решает `resolve_decor_inset_viewport`.
    pub fn has_decor_insets(&self) -> bool {
        matches!(self.reset_text_box.td_inset, Some(Some(_)))
            || self.reset_text_box.decors.iter().any(|d| d.inset.is_some())
    }

    pub fn resolve_decor_inset_viewport(&mut self, viewport: (f32, f32), vcx: ValueCx<'_>) {
        let vp = self.viewport_cx(viewport);
        let fix = |length: &mut Option<Len>| {
            if let Some(v) = *length {
                *length = Some(v.fold_viewport(vp, vcx));
            }
        };
        self.resolve_decor_inset_lengths(fix, vcx);
    }

    pub(super) fn resolve_decor_inset_lengths(
        &mut self,
        fix: impl Fn(&mut Option<Len>) + Copy,
        vcx: ValueCx<'_>,
    ) {
        walk_decor_inset_lengths(self, fix, vcx);
    }
}

/// Обход отступов линий оформления: общий для записи и проверки без записи.
pub(super) fn walk_decor_inset_lengths<T: Target>(
    target: &mut T,
    fix: impl Fn(&mut Option<Len>) + Copy,
    vcx: ValueCx<'_>,
) {
    let map = |pair: [DecorLen; 2]| pair.map(|value| map_length(value, fix, vcx));
    step(target, |s| {
        let now = s.reset_text_box.td_inset??;
        let next = map(now);
        (next != now).then_some(Change::DecorInset(next))
    });
    step(target, |s| {
        let decors = &s.reset_text_box.decors;
        if decors.iter().all(|decor| decor.inset.is_none()) {
            return None;
        }
        let next: Vec<_> = decors.iter().map(|decor| decor.inset.map(map)).collect();
        next.iter()
            .ne(decors.iter().map(|decor| &decor.inset))
            .then_some(Change::DecorInsets(next))
    });
}
