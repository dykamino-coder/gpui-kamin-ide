//! Computed::computed_font_px: вычисленный кегль элемента (css-fonts-4
//! §font-size-prop: «an absolute length») из вычисленного кегля родителя.
//! Один расчёт на два места: слияние со стилем родителя (`resolve_em`) и
//! фаза шрифта корня, от которой считаются `rem`/`rlh` (css-values-4
//! §6.1.4, `style::cascade::compute_cx`).

use super::*;
use crate::document::state::ValueCx;
use crate::style::values::value::calc_get;

/// Таблица ключевых кеглей при базовом 16px (CSS 2.1 §15.7, css-fonts-4
/// §absolute-size): по ней шагают `larger`/`smaller`.
const KEYWORD_PX: [f32; 8] = [9.0, 10.0, 13.0, 16.0, 18.0, 24.0, 32.0, 48.0];

impl Computed {
    /// Вычисленный кегль в точках при кегле родителя `parent_px`; `family` —
    /// семейство для замера `ch`/`ex`/`ic`/`cap`. `None` — кегль не задан
    /// (наследуется) или зависит от недоступной метрики (`lh` в `calc()`
    /// либо единицы окна до появления размеров viewport).
    ///
    /// Единицы шрифта в самом `font-size` меряются кеглем РОДИТЕЛЯ
    /// (css-values-4 §6.1.1: «when used in the value of the font-size
    /// property … they resolve against the computed metrics of the parent
    /// element»); процент — доля кегля родителя (css-fonts-4).
    pub(crate) fn computed_font_px(
        &self,
        family: &str,
        parent_px: f32,
        vcx: ValueCx<'_>,
    ) -> Option<f32> {
        // `larger`/`smaller` шагают по таблице, если кегль родителя в ней
        // стоит (§15.7); иначе работает запасной множитель `Len::Em`.
        if self.font_size_step != 0
            && let Some(i) = KEYWORD_PX.iter().position(|t| (t - parent_px).abs() < 0.01)
            && let Ok(j) = usize::try_from(i as i32 + i32::from(self.font_size_step))
            && let Some(px) = KEYWORD_PX.get(j)
        {
            return Some(*px);
        }
        let metrics = || crate::style::font_metrics::horizontal(family, parent_px, vcx);
        let ic = || crate::style::font_metrics::ideographic(family, parent_px, vcx);
        let size = self.font_size?;
        let size = if vcx.viewport.0 > 0.0 && vcx.viewport.1 > 0.0 {
            size.fold_viewport(self.viewport_cx(vcx.viewport), vcx)
        } else {
            size
        };
        Some(match size {
            Len::Px(px) => px,
            Len::Em(k) | Len::Pct(k) => k * parent_px,
            Len::EmPx(k, add) => k * parent_px + add,
            Len::Ch(k) => k * metrics().0,
            Len::Ex(k) => k * metrics().1,
            Len::Ic(k) => k * ic(),
            Len::Calc(id) => {
                let s = calc_get(id, vcx);
                if s.lh != 0.0 || s.vh != 0.0 || s.vw != 0.0 || s.ext.iter().any(|v| *v != 0.0) {
                    return None;
                }
                let (ch, ex) = metrics();
                let cap = if s.cap != 0.0 {
                    crate::style::font_metrics::vertical(family, parent_px, vcx).2
                } else {
                    0.0
                };
                let ic = if s.ic != 0.0 { ic() } else { 0.0 };
                // css-values-4 §10.12: результат вне допустимого диапазона
                // свойства прижимается к нему — кегль не отрицателен.
                (s.px
                    + (s.em + s.pct) * parent_px
                    + s.ch * ch
                    + s.ex * ex
                    + s.ic * ic
                    + s.cap * cap)
                    .max(0.0)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn computed(font_size: &str) -> (Computed, std::rc::Rc<crate::document::state::DocState>) {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let mut c = Computed::default();
        c.apply_one("font-size", font_size, doc.values());
        (c, doc)
    }

    #[test]
    fn font_relative_units_in_font_size_use_the_parent_size() {
        for (value, expected) in [
            ("20px", Some(20.0)),
            ("2em", Some(40.0)),
            ("150%", Some(30.0)),
            ("calc(1em + 4px)", Some(24.0)),
            ("calc(50% + 1em + 2px)", Some(32.0)),
            ("larger", Some(24.0)),
            ("10vw", None),
        ] {
            let (c, doc) = computed(value);
            let parent = if value == "larger" { 18.0 } else { 20.0 };
            assert_eq!(
                c.computed_font_px("", parent, doc.values()),
                expected,
                "{value}"
            );
        }
        let (c, doc) = computed("calc(1rem + 1em)");
        assert_eq!(c.computed_font_px("", 16.0, doc.values()), Some(32.0));
        assert_eq!(
            Computed::default().computed_font_px("", 16.0, doc.values()),
            None
        );
    }
}
