//! Входы окна, от которых зависит записанный кадр документа: границы,
//! маска, наследуемый стиль текста, смещение, масштаб, rem, эпоха ввода и
//! фокуса, активность и вид окна, время снимка анимаций, reduce-motion.

use gpui::{App, Bounds, ContentMask, Pixels, Point, TextStyle, Window, WindowAppearance};

/// Входы окна, от которых зависит записанный кадр.
#[derive(Clone, PartialEq)]
pub(super) struct WindowKey {
    bounds: Bounds<Pixels>,
    content_mask: ContentMask<Pixels>,
    text: TextStyle,
    offset: Point<Pixels>,
    scale: f32,
    rem: Pixels,
    interaction: u64,
    active: bool,
    appearance: WindowAppearance,
    elapsed: Option<std::time::Duration>,
    reduce_motion: bool,
    animation: (Option<std::time::Duration>, u64),
}

impl WindowKey {
    /// Discount inputs that only moved the root scroller
    /// (`RootScroll::key`); the frame replays them through its scroll nodes.
    pub(super) fn capture(
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &App,
        state: &super::DocState,
    ) -> Self {
        let scroll_inputs = state
            .root_scroll
            .as_ref()
            .map_or(0, |root| root.scroll_inputs.get());
        state.animation.sample(window, cx);
        Self {
            bounds,
            content_mask: window.content_mask(),
            text: window.text_style(),
            offset: window.element_offset(),
            scale: window.scale_factor(),
            rem: window.rem_size(),
            interaction: window.interaction_epoch().wrapping_sub(scroll_inputs),
            active: window.is_window_active(),
            appearance: window.appearance(),
            elapsed: cx.try_global::<gpui::AnimationElapsedTime>().map(|t| t.0),
            reduce_motion: cx.reduce_motion(),
            animation: state.animation.retained_key(),
        }
    }
}
