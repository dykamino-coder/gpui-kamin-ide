//! Маски и преобразования при обходе детей через явный стек продолжений.

use super::retained::InkProbe;
use super::*;

impl Window {
    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_content_mask_async<R>(
        &mut self,
        mask: Option<ContentMask<Pixels>>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        let depth = self.content_mask_stack.len();
        let _clip_scope = self.push_content_mask(mask);
        self.with_restore_async(
            depth,
            |window, depth| window.content_mask_stack.truncate(*depth),
            f,
        )
        .await
        .0
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_content_mask_replaced_async<R>(
        &mut self,
        mask: ContentMask<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        let mask = self.scaled_mask(mask);
        let _clip_scope = self.segment_clip_scope(mask, true);
        let depth = self.content_mask_stack.len();
        self.content_mask_stack.push(mask);
        self.with_restore_async(
            depth,
            |window, depth| window.content_mask_stack.truncate(*depth),
            f,
        )
        .await
        .0
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_content_mask_exact_async<R>(
        &mut self,
        mask: ContentMask<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        let _clip_scope = self.segment_clip_scope(mask, true);
        let depth = self.content_mask_stack.len();
        self.content_mask_stack.push(mask);
        self.with_restore_async(
            depth,
            |window, depth| window.content_mask_stack.truncate(*depth),
            f,
        )
        .await
        .0
    }

    /// Отображает маски детей тем же подобием до завершения продолжения.
    pub async fn with_mask_scale_async<R>(
        &mut self,
        origin: Point<Pixels>,
        scale: f32,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.mask_scale.replace((origin, scale));
        self.with_restore_async(
            previous,
            |window, previous| window.mask_scale = *previous,
            f,
        )
        .await
        .0
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_transformation_async<R>(
        &mut self,
        transformation: TransformationMatrix,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        // KaminIDE patch: вложение `outer · inner` (css-transforms-1
        // §transform-rendering: матрица предка применяется ПОСЛЕ своей; Blink
        // GeometryMapper перемножает parent·local). Прежний `inner · outer`
        // ломал вложенные поворот/скос (`transform-compound-001`); вертикальный
        // `CombinedUpright` переводит свою четверть оборота в локальные точки
        // строки (`combined_geometry::in_local_space`). При пустом стеке
        // (`current` = unit) результат побайтно прежний: 1·a + 0·c = a.
        let combined = self.current_transformation().compose(transformation);
        // KaminIDE patch: обычное вложение маски не переводит — ни своё, ни
        // унаследованное от `with_transformation_masked` снаружи: вертикальный
        // абзац, `CombinedUpright`, стопка страниц и объёмный путь рисуются
        // байт в байт как прежде.
        self.with_transform_frame_async(combined, None, f).await
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_transformation_masked_async<R>(
        &mut self,
        transformation: TransformationMatrix,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let combined = self.current_transformation().compose(transformation);
        let r = combined.rotation_scale;
        let keeps_axes = (r[0][1].abs() < 1e-4 && r[1][0].abs() < 1e-4)
            || (r[0][0].abs() < 1e-4 && r[1][1].abs() < 1e-4);
        self.with_transform_frame_async(combined, keeps_axes.then_some(combined), f)
            .await
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn paint_bottom_layer_async<R>(&mut self, f: impl AsyncFnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_paint();
        self.next_frame.scene.push_bottom_layer();
        self.with_restore_async((), |window, _| window.next_frame.scene.pop_layer(), f)
            .await
            .0
    }

    pub(super) async fn with_transform_frame_async<R>(
        &mut self,
        transformation: TransformationMatrix,
        mask_map: Option<TransformationMatrix>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.transformation_stack.len();
        self.transformation_stack.push(transformation);
        let previous_map = mem::replace(&mut self.mask_map, mask_map);
        self.with_restore_async(
            (depth, previous_map),
            |window, previous| {
                window.transformation_stack.truncate(previous.0);
                window.mask_map = previous.1;
            },
            f,
        )
        .await
        .0
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_ink_sink_async<R>(
        &mut self,
        sink: Rc<InkSink>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.next_frame.scene.ink.push(sink);
        let probe = InkProbe::open(self);
        let result = f(self).await;
        if let Some(probe) = probe {
            probe.close(self);
        }
        self.next_frame.scene.ink.pop();
        result
    }
    // Геометрия слоя вычисляется одинаково для обоих способов исполнения области.
    pub(super) fn push_paint_layer(&mut self, bounds: Bounds<Pixels>) -> bool {
        self.invalidator.debug_assert_paint();

        let content_mask = self.content_mask();
        let clipped_bounds = bounds.intersect(&content_mask.bounds);
        if !clipped_bounds.is_empty() {
            // KaminIDE patch: порядок слоя считается по МЕСТУ НА ЭКРАНЕ.
            // Внутри повёрнутого текста границы слоя до-трансформные — дерево
            // границ видело его в чужой клетке, и фон соседа получал порядок
            // ПОВЕРХ глифов (table-cell-align-005).
            // KaminIDE patch: exact layer bounds, not upstream's floor/ceil
            // cover: a layer grown to the next device pixel intersects the
            // neighbour run's glyphs and is ordered above them (a run's
            // background then covered the previous glyph's AA column,
            // `inline-formatting-context-002`).
            let mut scaled = clipped_bounds.scale(self.scale_factor());
            let m = self.current_transformation();
            if m != TransformationMatrix::unit() {
                let b = scaled;
                let corners = [
                    (b.origin.x.0, b.origin.y.0),
                    (b.origin.x.0 + b.size.width.0, b.origin.y.0),
                    (b.origin.x.0, b.origin.y.0 + b.size.height.0),
                    (
                        b.origin.x.0 + b.size.width.0,
                        b.origin.y.0 + b.size.height.0,
                    ),
                ];
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for (x, y) in corners {
                    let tx =
                        m.translation[0] + m.rotation_scale[0][0] * x + m.rotation_scale[0][1] * y;
                    let ty =
                        m.translation[1] + m.rotation_scale[1][0] * x + m.rotation_scale[1][1] * y;
                    x0 = x0.min(tx);
                    y0 = y0.min(ty);
                    x1 = x1.max(tx);
                    y1 = y1.max(ty);
                }
                scaled = Bounds {
                    origin: point(ScaledPixels(x0), ScaledPixels(y0)),
                    size: size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)),
                };
            }
            self.next_frame.scene.push_layer(scaled);
        }

        !clipped_bounds.is_empty()
    }

    /// Рисует слой, удерживая его область до завершения продолжения.
    pub async fn paint_layer_async<R>(
        &mut self,
        bounds: Bounds<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let pushed = self.push_paint_layer(bounds);
        self.with_restore_async(
            pushed,
            |window, pushed| {
                if *pushed {
                    window.next_frame.scene.pop_layer();
                }
            },
            f,
        )
        .await
        .0
    }
}
