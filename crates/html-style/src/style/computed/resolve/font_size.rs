//! Computed::computed_font_px: вычисленный кегль элемента (css-fonts-4
//! §font-size-prop: «an absolute length») из вычисленного кегля родителя.
//! Один расчёт на два места: слияние со стилем родителя (`resolve_em`) и
//! фаза шрифта корня, от которой считаются `rem`/`rlh` (css-values-4
//! §6.1.4, `style::cascade::compute_cx`).

use super::*;
use crate::style::values::value::calc_get;
use crate::values::ValueCx;

impl Computed {
    /// Вычисленный кегль из computed размера и used метрик родителя `font`. `None` — кегль не задан
    /// (наследуется) или зависит от недоступной метрики (`lh` в `calc()`
    /// либо единицы окна до появления размеров viewport).
    ///
    /// Единицы шрифта в самом `font-size` меряются кеглем РОДИТЕЛЯ
    /// (css-values-4 §6.1.1: «when used in the value of the font-size
    /// property … they resolve against the computed metrics of the parent
    /// element»); процент — доля кегля родителя (css-fonts-4).
    pub fn computed_font_px(&self, font: FontCx<'_>, vcx: ValueCx<'_>) -> Option<f32> {
        let parent_px = font.font_px;
        let family = font.family;
        // Relative keywords scale the computed parent size, including when
        // that size happens to equal an absolute-size keyword table entry.
        if self.reset_cascade.font_size_step != 0 {
            return Some(
                parent_px
                    * if self.reset_cascade.font_size_step > 0 {
                        1.2
                    } else {
                        5. / 6.
                    },
            );
        }
        let metrics = || {
            font.units.map_or_else(
                || crate::style::font_metrics::horizontal(family, font.metric_px, vcx),
                |units| (units[1], units[2]),
            )
        };
        let ic = || {
            font.units.map_or_else(
                || crate::style::font_metrics::ideographic(family, font.metric_px, vcx),
                |units| units[3],
            )
        };
        let size = self.inh.font.font_size?;
        // Without a viewport the size stays unfolded, which is a read of it too.
        if size.folds_viewport(vcx) {
            crate::values::viewport_reads::note(vcx);
        }
        let size = if vcx.viewport.0 > 0.0 && vcx.viewport.1 > 0.0 {
            size.fold_viewport(self.viewport_cx(vcx.viewport, vcx), vcx)
        } else {
            size
        };
        // A `clamp(2rem, 1rem + 5vw, 4rem)` tree: the viewport is folded above,
        // font leaves of `font-size` resolve against the PARENT font size.
        let size = size.resolve_math(vcx, |s| {
            s.fold_font(parent_px, || {
                let (ch, ex) = metrics();
                let cap = font.units.map_or_else(
                    || crate::style::font_metrics::vertical(family, font.metric_px, vcx).2,
                    |units| units[0],
                );
                [cap, ch, ex, ic()]
            })
        });
        Some(match size {
            // A folded tree may go below zero: the font size is clamped to the
            // property range (§10.12).
            Len::Px(px) => px.max(0.0),
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
                    font.units.map_or_else(
                        || crate::style::font_metrics::vertical(family, font.metric_px, vcx).2,
                        |units| units[0],
                    )
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

    fn computed(font_size: &str) -> (Computed, std::rc::Rc<crate::values::DocumentValues>) {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
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
            ("larger", Some(21.6)),
            ("10vw", None),
        ] {
            let (c, doc) = computed(value);
            let parent = if value == "larger" { 18.0 } else { 20.0 };
            assert_eq!(
                c.computed_font_px(FontCx::new("", parent), doc.values()),
                expected,
                "{value}"
            );
        }
        let (c, doc) = computed("calc(1rem + 1em)");
        assert_eq!(
            c.computed_font_px(FontCx::FALLBACK, doc.values()),
            Some(32.0)
        );
        assert_eq!(
            Computed::default().computed_font_px(FontCx::FALLBACK, doc.values()),
            None
        );
    }
}
