//! Краска стопки листов через общий стек продолжений.

use super::*;

impl PageStack {
    pub(super) fn paint_sheets_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        window.continuations().pin(async move {
            if !self.has_complete_plan() || self.oof.plan.borrow().failure.is_some() {
                return;
            }
            // Each native source is painted exactly once; sheets replay its records.
            if self.record_native(window, cx, stack).await.is_err() {
                return;
            }
            let s = self.grid.get().1;
            let pages = self.pages.get();
            // Масштаб вокруг левого верха стопки; матрица — в точках устройства,
            // как у `paint::effects::Transformed`.
            let k = window.scale_factor();
            let dev = |v: f32| px(v).scale(k);
            let origin = point(
                dev(f32::from(bounds.origin.x)),
                dev(f32::from(bounds.origin.y)),
            );
            let back = point(
                dev(-f32::from(bounds.origin.x)),
                dev(-f32::from(bounds.origin.y)),
            );
            let matrix = gpui::TransformationMatrix::unit()
                .translate(origin)
                .compose(gpui::TransformationMatrix {
                    rotation_scale: [[s, 0.0], [0.0, s]],
                    translation: [0.0, 0.0],
                })
                .translate(back);
            let plan = self.plan.borrow().clone();
            let masks: Vec<gpui::ContentMask<Pixels>> =
                (0..pages).map(|i| self.area_mask(bounds, i)).collect();
            let rect = |x: f32, y: f32, w: f32, h: f32| Bounds {
                origin: point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
                size: size(px(w), px(h)),
            };
            // Main copies record before the sheet matrix, in unscaled stack points.
            let replayed = self.record_replays(bounds, &plan, window, cx, stack).await;
            window
                .with_transformation_async(matrix, async |window| {
                    let masks = (masks, &replayed);
                    self.paint_sheets(bounds, cx, s, pages, &plan, masks, rect, window, stack)
                        .await;
                })
                .await;
            // Повёрнутые листы (`page-orientation`): тот же порядок слоёв, но под
            // СВОЕЙ матрицей — поворот на четверть оборота вокруг листа, затем
            // масштаб стопки. Лист разложен в ячейке как неповёрнутый (левый верх
            // `O`); маски детей и фрагментов заданы в его непреобразованных
            // точках и едут вместе с ним (`with_transformation_masked`: поворот на
            // четверть оборота оси сохраняет).
            self.paint_page_contents(bounds, window, cx, pages, k, plan.fragments, rect, stack)
                .await;
        })
    }
}
