//! Обход paint детей BandFlow через общий стек продолжений.

use super::*;

impl BandFlow {
    pub(super) async fn paint_children(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        // The host is not a stacking context (CSS 2.1 Appendix E.2): later
        // block backgrounds of the enclosing context paint before the
        // inline foreground of its runs. Paint groups keep their own
        // boundary (`holders`); the outline phase wraps the host (`Effect::OutlineFlow`).
        crate::hooks::paint_reopen_async(window, async |window| {
            for el in self.built.iter_mut() {
                el.paint_async(window, cx, stack).await;
            }
        })
        .await;
    }
}
