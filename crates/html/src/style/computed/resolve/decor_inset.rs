//! Разрешение длин `text-decoration-inset` (CSS Text Decoration 4 §4.1):
//! шрифтовые единицы и `lh` — по шрифту и высоте строки элемента после
//! наследования, единицы окна — отдельным проходом при сборке строки, у которой
//! общего прохода разрешения окна нет. Смесь доли и точек остаётся `DecorLen::Mix`.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, DecorLen};
use crate::style::values::value::{Len, calc_get};

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
        let line = match self.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k) | Len::Pct(k)) => k * font,
            _ => {
                let normal = crate::text::metrics::normal_line(family, vcx);
                if normal > 0.0 {
                    crate::text::metrics::normal_px(normal, font, vcx)
                } else {
                    1.2 * font
                }
            }
        };
        if let Some(Some(pair)) = &mut self.td_inset {
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
    pub(crate) fn resolve_decor_inset_viewport(&mut self, viewport: (f32, f32), vcx: ValueCx<'_>) {
        let insets = self.viewport_unit_insets.unwrap_or_default();
        let viewport = (
            (viewport.0 - insets[0]).max(0.0),
            (viewport.1 - insets[1]).max(0.0),
        );
        let vertical = self.wm().vertical();
        let container = Some((
            self.cq_box.0.unwrap_or(viewport.0),
            self.cq_box.1.unwrap_or(viewport.1),
        ));
        let fix = |length: &mut Option<Len>| match *length {
            Some(Len::Vw(k)) => *length = Some(Len::Px(k * viewport.0)),
            Some(Len::Vh(k)) => *length = Some(Len::Px(k * viewport.1)),
            Some(Len::Calc(i)) => {
                let mut sum = calc_get(i, vcx);
                if sum.fold_viewport(viewport, vertical, container) {
                    *length = sum.collapse_mixed(vcx);
                }
            }
            _ => {}
        };
        if let Some(Some(pair)) = &mut self.td_inset {
            for value in pair {
                *value = map_length(*value, fix, vcx);
            }
        }
        for decor in &mut self.decors {
            if let Some(pair) = &mut decor.inset {
                for value in pair {
                    *value = map_length(*value, fix, vcx);
                }
            }
        }
    }
}
