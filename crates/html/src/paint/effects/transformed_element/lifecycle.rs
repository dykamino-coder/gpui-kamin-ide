//! Жизненный цикл GPUI-элемента; отделён от состояния и конструкторов.

use super::painting::paint_body;
use crate::hooks::Hooks;

use super::Transformed;
use gpui::{App, Bounds, LayoutId, Pixels, Window, px};

impl Hooks for Transformed {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout_id: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        // Плоская матрица — на стек якорей (`anchor::tf_push`): рамку якоря
        // проба снимает на подготовке, а трансформ применяется только в
        // `paint`, и без стека якорь виделся до трансформа (css-anchor-
        // position-1 §2 «includes … transforms»; `transform-001/002/009`).
        // Формула та же, что у плоского пути `paint`, но в css-точках, без
        // `scale_factor`: x' = o + lin·(x − o) + сдвиг. Объёмный путь в стек
        // не идёт — его матрица решается на отрисовке по накопленной ячейке.
        let flat = !self.has_3d && self.frame_3d.is_none() && self.under_3d.is_none();
        // Чистый плоский сдвиг — смена начала координат (css-transforms-1
        // §transform-rendering): коробка обязана рисоваться байт в байт как
        // разложенная на сдвинутом месте. Матрицей дробный сдвиг устройства
        // (10px × 1.25) ложился ПОСЛЕ округления краёв раскладки и выбора
        // подпикселя глифов — края и текст расходились на точку с эталоном
        // на `top/left` (Blink так же проносит дробное смещение сквозь
        // 2D-сдвиг: `PaintPropertyTreeBuilder`, subpixel accumulation).
        // Поддерево переносится до округления (`set_layout_placed_origin`,
        // тот же механизм у `LatePlace`), края округляются на конечном месте.
        self.placed = false;
        self.exact_origin = Some(window.layout_origin_unrounded(*layout_id));
        // A continued root presented per fragment: each placement applies its
        // own fragment's matrix, so the source paints it untransformed.
        if self
            .node
            .is_some_and(|node| crate::paint::coordinator::segments::presented_root(node, window))
        {
            self.placed = true;
            self.child
                .prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
            return;
        }
        if let Some(r) = self.ref_box.as_ref() {
            let own = gpui::Bounds {
                origin: window.layout_origin_unrounded(*layout_id),
                size: window.layout_size_unrounded(*layout_id),
            };
            r.set(Some(r.get().map_or(own, |u| u.union(&own))));
        }
        if flat && self.perspective.is_none() {
            let size = window.layout_size_unrounded(*layout_id);
            if let Some((sx, sy)) = self.pure_shift(f32::from(size.width), f32::from(size.height)) {
                let origin = window.layout_origin_unrounded(*layout_id);
                window.set_layout_placed_origin(*layout_id, origin + gpui::point(px(sx), px(sy)));
                self.placed = true;
                self.child
                    .prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
                return;
            }
        }
        let mut hit = None;
        if flat {
            let (w, h) = {
                let exact = window.layout_size_unrounded(*layout_id);
                (f32::from(exact.width), f32::from(exact.height))
            };
            let origin = self.scaled_origin(bounds.origin);
            let ox = f32::from(origin.x) + w * self.origin.0 + self.origin_px.0.unwrap_or(0.0);
            let oy = f32::from(origin.y) + h * self.origin.1 + self.origin_px.1.unwrap_or(0.0);
            let sx = self.tr[0][0] + w * self.tr[0][1] + h * self.tr[0][2];
            let sy = self.tr[1][0] + w * self.tr[1][1] + h * self.tr[1][2];
            let [[a, b], [c, d]] = self.lin;
            let translation = [ox - a * ox - b * oy + sx, oy - c * ox - d * oy + sy];
            crate::layout::positioned::anchor::tf_push(
                [[a, b, translation[0]], [c, d, translation[1]]],
                self.values.vcx(),
            );
            // The same flat matrix maps the pointer back into the subtree's
            // untransformed hitboxes (css-transforms-1 §transform-rendering:
            // hit testing follows the rendered, transformed shape).
            hit = Some(gpui::TransformationMatrix {
                rotation_scale: self.lin,
                translation,
            });
        }
        match hit {
            Some(matrix) => window.with_hitbox_transform(matrix, |window| {
                self.child.prepaint(window, cx);
            }),
            None => {
                self.child.prepaint(window, cx);
            }
        }
        if flat {
            crate::layout::positioned::anchor::tf_pop(self.values.vcx());
        }
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout_id: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        paint_body(self, bounds, layout_id, _prepaint, window, cx)
    }
}
