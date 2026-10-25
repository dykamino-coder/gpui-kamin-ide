//! Геометрическое окружение исходного слоя для завершения у ICB.

use crate::document::state::ValueDoc;
use crate::layout::positioned::anchor::TransformContext;
use gpui::{ContentMask, PaintCtx, Pixels, Window};

#[derive(Clone)]
pub(crate) struct Context {
    pub(super) mask: ContentMask<Pixels>,
    paint: PaintCtx,
    anchors: Option<TransformContext>,
}

impl Context {
    pub(super) fn capture(values: Option<ValueDoc>, window: &Window) -> Self {
        Self {
            mask: window.content_mask(),
            paint: window.paint_ctx(),
            anchors: values.map(TransformContext::capture),
        }
    }

    pub(super) fn enter<R>(
        &self,
        window: &mut Window,
        f: impl FnOnce(&mut Window) -> R,
    ) -> Result<R, gpui::PaintError> {
        // The original host's id/text/render scopes must accompany geometry:
        // the completed element is painted back under that host's ancestors.
        let run = || window.with_paint_ctx(self.paint.clone(), f);
        match &self.anchors {
            Some(anchors) => anchors.enter(run),
            None => run(),
        }
    }
}
