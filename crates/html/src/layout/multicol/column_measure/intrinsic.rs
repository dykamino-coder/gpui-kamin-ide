//! Внутренние ширины колонок измеряются на общем явном стеке потомков.
use super::*;
impl ColumnStack {
    pub(super) async fn intrinsic_widths(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Option<(f32, f32)> {
        let axis = self.axis;
        let count = self.count;
        let gap = self.gap;
        let deferred_intrinsic = self.deferred.is_some();
        let Intrinsic(col_w) = self.intrinsic.filter(|_| !axis.is_vertical())?;

        // A deferred stack builds its children for this only: an
        // unmeasured container never needs them.
        let mut intrinsic_children = self
            .deferred
            .as_ref()
            .map(|r| r.intrinsic_children())
            .unwrap_or_default();
        let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
        let (mut span_min, mut span_max) = (0.0f32, 0.0f32);
        for (el, span) in self
            .children
            .iter_mut()
            .filter(|_| !deferred_intrinsic)
            .map(|c| (&mut c.el, c.span))
            .chain(intrinsic_children.iter_mut().map(|el| (el, false)))
        {
            let mn = f32::from(
                el.layout_as_root_unrounded_async(
                    size(
                        gpui::AvailableSpace::MinContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                    stack,
                )
                .await
                .width,
            );
            let mx = f32::from(
                el.layout_as_root_unrounded_async(
                    size(
                        gpui::AvailableSpace::MaxContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                    stack,
                )
                .await
                .width,
            );
            // Спаннер идёт во всю ширину коробки: на число колонок он не
            // умножается, а лишь ПОДНИМАЕТ итог — Blink
            // `ComputeSpannersMinMaxSizes` (:523) через
            // `MinMaxSizes::Encompass` (`min_max_sizes.h:26`, это `max`
            // по обеим границам).
            if span {
                span_min = span_min.max(mn);
                span_max = span_max.max(mx);
            } else {
                kid_min = kid_min.max(mn);
                kid_max = kid_max.max(mx);
            }
        }
        let n = count.max(1) as f32;
        let gap_extra = gap * (n - 1.0);
        let (mut mn, mut mx) = (kid_min, kid_max);
        match col_w.filter(|w| *w > 0.0) {
            // «The min-content inline size of a multi-column container
            // with a computed column-width not auto is the smaller of its
            // column-width and the largest min-content inline-size
            // contribution of its contents.»
            Some(w) => {
                mn = mn.min(w);
                mx = mx.max(w).max(mn);
            }
            // «…with a computed column-width of auto is the largest
            // min-content inline-size contribution of its contents
            // multiplied by its column-count …, plus its column-gap
            // multiplied by column-count minus 1.» При ЗАДАННОЙ ширине
            // колонки минимум на число колонок не умножается (Blink
            // :482 — «column-count … is ignored in intrinsic min
            // inline-size calculation, if column-width is specified»).
            None => mn = mn * n + gap_extra,
        }
        mx = mx * n + gap_extra;
        Some((mn.max(span_min), mx.max(span_max)))
    }
}
