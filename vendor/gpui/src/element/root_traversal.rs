//! Подготовка самостоятельных корней использует общий стек запроса и prepaint.

use super::*;

impl AnyElement {
    /// Запрашивает новый корень через явный стек перед его раскладкой.
    pub async fn layout_as_root_async(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Size<Pixels> {
        if self.0.root_layout_id().is_none() {
            self.request_layout_async(window, cx, stack).await;
        }
        child_traversal::run_child(stack, |stack| {
                self.0
                    .layout_as_root_async(available_space, window, cx, stack)
        })
        .await
    }

    /// Запрашивает измерительный корень через явный стек.
    pub async fn snapshot_layout_measurement_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> crate::LayoutMeasurement {
        let root = match self.0.root_layout_id() {
            Some(root) => root,
            None => self.request_layout_async(window, cx, stack).await,
        };
        window.snapshot_layout_measurement(root)
    }

    /// Возвращает размер самостоятельного корня до округления.
    pub async fn layout_as_root_unrounded_async(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Size<Pixels> {
        self.layout_as_root_async(available_space, window, cx, stack)
            .await;
        let root = self.0.root_layout_id().expect("layout root was computed");
        window.layout_size_unrounded(root)
    }

    /// Возвращает размер и content overflow самостоятельного корня.
    pub async fn layout_as_root_with_content_async(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> (Size<Pixels>, Size<Pixels>) {
        self.layout_as_root_async(available_space, window, cx, stack)
            .await;
        let root = self.0.root_layout_id().expect("layout root was computed");
        (
            window.layout_size_unrounded(root),
            window.layout_content_size_unrounded(root),
        )
    }

    /// Раскладывает самостоятельный корень с абсолютным началом округления.
    pub async fn layout_as_root_at_async(
        &mut self,
        origin: Point<Pixels>,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Size<Pixels> {
        self.layout_as_root_async(available_space, window, cx, stack)
            .await;
        let root = self.0.root_layout_id().expect("layout root was computed");
        window.set_layout_root_origin(root, origin);
        window.layout_bounds(root).size
    }

    /// Удерживает абсолютное смещение до завершения дочернего prepaint.
    pub async fn prepaint_at_async(
        &mut self,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Option<FocusHandle> {
        window
            .with_absolute_element_offset_async(origin, async |window| {
                self.prepaint_async(window, cx, stack).await
            })
            .await
    }

    /// Раскладывает корень и подготавливает его по абсолютным координатам.
    pub async fn prepaint_as_root_async(
        &mut self,
        origin: Point<Pixels>,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Option<FocusHandle> {
        self.layout_as_root_async(available_space, window, cx, stack)
            .await;
        self.prepaint_at_async(origin, window, cx, stack).await
    }
}
