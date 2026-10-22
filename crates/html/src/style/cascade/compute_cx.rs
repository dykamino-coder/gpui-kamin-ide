//! ComputeCx: явные входы вычисления стиля элемента между фазами
//! ([`Phase`](super::phase::Phase)). Свойство поздней фазы, зависящее от
//! ранней, получает её итог отсюда, а не читает наполовину применённый
//! `Computed` (Blink `StyleResolverState` после `ApplyCascadeAffecting`/
//! `UpdateFont`, Stylo `computed::Context` с `builder.writing_mode`).
//!
//! Итоги фаз:
//! * письмо — после фазы письма; по нему логические сокращения сторон
//!   сопоставляются с физическими (css-logical-1 §4);
//! * метрики корня — после фазы шрифта корневого элемента: база `rem`,
//!   `rlh`, `rex`/`rch`/`ric`/`rcap` (css-values-4 §6.1.4). Само
//!   `font-size` корня меряет эти единицы начальными значениями, поэтому
//!   до фазы шрифта корня метрики сбрасываются к начальным.

use super::phase::Phase;
use super::vars::CascadeCx;
use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::values::value::{
    Len, RootMetrics, root_normal_line_px, set_root_family, set_root_font_view, set_root_metrics,
};

/// Входы поздних фаз вычисления одного элемента.
#[derive(Clone, Copy)]
pub(crate) struct ComputeCx<'a> {
    pub vcx: ValueCx<'a>,
    pub element: CascadeCx<'a>,
    /// Письмо элемента — итог фазы письма.
    pub wm: WritingMode,
}

impl<'a> ComputeCx<'a> {
    pub fn new(element: CascadeCx<'a>, vcx: ValueCx<'a>) -> Self {
        ComputeCx {
            vcx,
            element,
            wm: WritingMode::default(),
        }
    }

    /// Перед фазой `phase`: корень меряет `rem` в своём `font-size`
    /// начальными метриками (css-values-4 §6.1.4: «When specified in the
    /// font-size property of the root element … the rem units refer to the
    /// property's initial value»), а не оставшимися от прошлого обхода.
    pub fn before(&mut self, phase: Phase) {
        if phase == Phase::Font && self.element.root {
            *self.vcx.doc.root.borrow_mut() = RootMetrics::default();
        }
    }

    /// После фазы `phase`: её итоги становятся входами следующих.
    pub fn after(&mut self, phase: Phase, style: &Computed) {
        match phase {
            Phase::CascadeAffecting => self.wm = style.wm(),
            Phase::Font if self.element.root => set_root_from(style, self.vcx),
            _ => {}
        }
    }
}

/// Метрики корня по итогу его фазы шрифта: вычисленный кегль (родитель —
/// начальные 16px), высота строки и семейство.
fn set_root_from(style: &Computed, vcx: ValueCx<'_>) {
    let family = style.font_family.clone().unwrap_or_default();
    let font = style.computed_font_px(&family, 16.0, vcx).unwrap_or(16.0);
    let line = match style.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * font,
        _ => root_normal_line_px(&family, font, vcx),
    };
    set_root_metrics(vcx.doc, font, line);
    set_root_family(vcx.doc, &family);
    // Кегль в единицах окна: точек окна на каскаде ещё нет, `rem` доживает
    // той же единицей окна.
    set_root_font_view(
        vcx.doc,
        match style.font_size {
            Some(Len::Vh(k)) => Some((true, k)),
            Some(Len::Vw(k)) => Some((false, k)),
            _ => None,
        },
    );
}
