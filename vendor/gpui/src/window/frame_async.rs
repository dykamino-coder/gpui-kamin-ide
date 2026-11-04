//! Области стиля и координат живут до завершения дочерней фазы.

use super::*;

impl Window {
    /// Удерживает область до завершения дочернего продолжения.
    pub async fn with_text_style_async<R>(
        &mut self,
        style: Option<TextStyleRefinement>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        if let Some(style) = style {
            let inner = self.text_style_stack.push(style);
            let outer = std::mem::replace(&mut self.text_style_stack, inner);
            self.with_restore_async(
                outer,
                |window, outer| window.text_style_stack = std::mem::take(outer),
                f,
            )
            .await
            .0
        } else {
            f(self).await
        }
    }

    /// Удерживает область до завершения дочернего продолжения.
    pub async fn with_absolute_element_offset_async<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let depth = self.element_offset_stack.len();
        self.element_offset_stack.push(offset);
        self.with_restore_async(
            depth,
            |window, depth| window.element_offset_stack.truncate(*depth),
            f,
        )
        .await
        .0
    }

    /// Удерживает область до завершения дочернего продолжения.
    pub async fn with_element_offset_async<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();

        if offset.is_zero() {
            return f(self).await;
        };

        let abs_offset = self.element_offset() + offset;
        self.with_absolute_element_offset_async(abs_offset, f).await
    }

    /// Удерживает область до завершения дочернего продолжения.
    pub async fn with_exact_element_offset_async<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let depth = self.exact_element_offset_depth;
        self.exact_element_offset_depth += 1;
        let abs_offset = self.element_offset() + offset;
        self.with_restore_async(
            depth,
            |window, depth| window.exact_element_offset_depth = *depth,
            async |window| {
                window
                    .with_absolute_element_offset_async(abs_offset, f)
                    .await
            },
        )
        .await
        .0
    }

    /// Удерживает область до завершения дочернего продолжения.
    pub async fn with_optional_element_state_async<S, R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl AsyncFnOnce(Option<Option<S>>, &mut Self) -> (R, Option<S>),
    ) -> R
    where
        S: 'static,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        if let Some(global_id) = global_id {
            self.with_element_state_async(global_id, async |state, cx| {
                let (result, state) = f(Some(state), cx).await;
                let state =
                    state.expect("you must return some state when you pass some element id");
                (result, state)
            })
            .await
        } else {
            let (result, state) = f(None, self).await;
            debug_assert!(
                state.is_none(),
                "you must not return an element state when passing None for the global id"
            );
            result
        }
    }

    /// Удерживает обратное преобразование hitbox до завершения prepaint потомков.
    pub async fn with_hitbox_transform_async<R>(
        &mut self,
        matrix: TransformationMatrix,
        prepaint: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let scope = Rc::new(hit_transform::HitScope {
            parent: self.hit_scope.clone(),
            inverse: matrix.inverse(),
            clip: self.content_mask().bounds,
        });
        let saved = Box::new((
            self.hit_scope.replace(scope),
            std::mem::take(&mut self.content_mask_stack),
        ));
        let radius = f32::MAX / 64.0;
        self.content_mask_stack.push(ContentMask {
            bounds: Bounds::new(
                point(px(-radius), px(-radius)),
                size(px(radius * 2.0), px(radius * 2.0)),
            ),
        });
        self.with_restore_async(
            saved,
            |window, saved| {
                window.hit_scope = saved.0.take();
                window.content_mask_stack = std::mem::take(&mut saved.1);
            },
            prepaint,
        )
        .await
        .0
    }
}
