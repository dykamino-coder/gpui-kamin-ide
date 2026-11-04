//! Области окружения сохраняются в продолжении, пока явный стек обходит потомков.

use super::*;

impl Window {
    pub(crate) async fn with_child_style_async<R>(
        &mut self,
        style: &Style,
        bounds: Bounds<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        let depth = (self.text_style_stack.clone(), self.content_mask_stack.len());
        if let Some(text) = style.text_style() {
            self.push_text_style_ref(text);
        }
        let _clip_scope = self.push_content_mask(style.overflow_mask_snapped(bounds, self));
        self.with_restore_async(
            depth,
            |window, depth| {
                window.text_style_stack = std::mem::take(&mut depth.0);
                window.content_mask_stack.truncate(depth.1);
            },
            f,
        )
        .await
        .0
    }

    pub(crate) async fn with_element_opacity_async<R>(
        &mut self,
        opacity: Option<f32>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();

        let Some(opacity) = opacity else {
            return f(self).await;
        };

        let previous_opacity = self.element_opacity;
        self.element_opacity = previous_opacity * opacity;
        self.with_restore_async(
            previous_opacity,
            |window, previous| window.element_opacity = *previous,
            f,
        )
        .await
        .0
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_tab_group_async<R>(
        &mut self,
        index: Option<isize>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        if let Some(index) = index {
            self.next_frame.tab_stops.begin_group(index);
            let result = f(self).await;
            self.next_frame.tab_stops.end_group();
            result
        } else {
            f(self).await
        }
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_image_cache_async<R>(
        &mut self,
        image_cache: Option<AnyImageCache>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        if let Some(image_cache) = image_cache {
            let depth = self.image_cache_stack.len();
            self.image_cache_stack.push(image_cache);
            self.with_restore_async(
                depth,
                |window, depth| window.image_cache_stack.truncate(*depth),
                f,
            )
            .await
            .0
        } else {
            f(self).await
        }
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_render_context_async<T: 'static, R>(
        &mut self,
        context: Option<Rc<T>>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let scope = self.render_contexts.enter(context);
        self.with_restore_async(scope, |_, _| {}, f).await.0
    }

    /// Выполняет область окна с продолжением на явном стеке обхода.
    pub async fn with_raster_context_async<T: std::any::Any, R>(
        &mut self,
        value: Option<Rc<T>>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.raster_frame.contexts.replace(value);
        self.with_restore_async(
            previous,
            |window, previous| {
                window.raster_frame.contexts.set(previous.take());
            },
            f,
        )
        .await
        .0
    }
    /// Удерживает подготовленную раскладку до завершения продолжения.
    pub async fn with_retained_layout_async<R>(
        &mut self,
        retained: &mut RetainedLayout,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let outer = self.activate_retained_layout(retained);
        self.with_restore_async(outer, Self::restore_retained_layout, f)
            .await
            .0
    }

    /// Удерживает цепочку идентификаторов до завершения продолжения.
    pub async fn with_element_id_context_async<R>(
        &mut self,
        context: &ElementIdContext,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let outer = self.replace_element_id_context(context);
        self.with_restore_async(
            outer,
            |window, outer| std::mem::swap(&mut window.element_id_stack, outer),
            f,
        )
        .await
        .0
    }
    /// Удерживает raster frame до завершения дочерней краски.
    pub async fn with_raster_frame_async<R>(
        &mut self,
        mut frame: RasterFrame,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        mem::swap(&mut self.raster_frame, &mut frame);
        self.with_restore_async(
            frame,
            |window, frame| mem::swap(&mut window.raster_frame, frame),
            f,
        )
        .await
        .0
    }
}
