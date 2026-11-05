//! Геометрическое окружение исходного слоя для завершения у ICB.

use crate::layout::layout_state::LayoutState;
use crate::layout::positioned::anchor::TransformContext;
use gpui::{ContentMask, PaintCtx, Pixels, Window};

#[derive(Clone)]
pub(crate) struct Context {
    pub(super) mask: ContentMask<Pixels>,
    paint: PaintCtx,
    anchors: Option<TransformContext>,
}

impl Context {
    pub(super) fn capture(layout: Option<std::rc::Rc<LayoutState>>, window: &Window) -> Self {
        Self {
            mask: window.content_mask(),
            paint: window.paint_ctx(),
            anchors: layout.map(TransformContext::capture),
        }
    }

    pub(super) async fn enter_async<R>(
        &self,
        window: &mut Window,
        f: impl AsyncFnOnce(&mut Window) -> R,
    ) -> Result<R, gpui::PaintError> {
        // The original host's id/text/render scopes must accompany geometry:
        // the completed element is painted back under that host's ancestors.
        let run = async || window.with_paint_ctx_async(self.paint.clone(), f).await;
        match &self.anchors {
            Some(anchors) => anchors.enter_async(run).await,
            None => run().await,
        }
    }
}
