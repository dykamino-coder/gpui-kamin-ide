//! Геометрическое окружение исходного слоя для завершения у ICB.

use crate::document::state::ValueDoc;
use crate::layout::positioned::anchor::TransformContext;
use gpui::{ContentMask, Pixels, TransformationMatrix, Window};

#[derive(Clone)]
pub(crate) struct Context {
    pub(super) mask: ContentMask<Pixels>,
    matrix: TransformationMatrix,
    anchors: Option<TransformContext>,
}

impl Context {
    pub(super) fn capture(values: Option<ValueDoc>, window: &Window) -> Self {
        Self {
            mask: window.content_mask(),
            matrix: window.current_transformation(),
            anchors: values.map(TransformContext::capture),
        }
    }

    pub(super) fn enter<R>(&self, window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        let current = window.current_transformation();
        let [[a, b], [c, d]] = current.rotation_scale;
        let [x, y] = current.translation;
        // GPUI nests matrices as parent · local. Reuse the canonical affine
        // inverse; a singular ancestor already suppresses this paint subtree.
        let local = if current == self.matrix {
            TransformationMatrix::unit()
        } else {
            crate::paint::effects::transformed_element::invert_affine([[a, b, x], [c, d, y]])
                .map(|inverse| inverse.compose(self.matrix))
                .unwrap_or_default()
        };
        let run = || {
            window.with_transformation(local, |window| {
                window.with_content_mask_replaced(self.mask, f)
            })
        };
        match &self.anchors {
            Some(anchors) => anchors.enter(run),
            None => run(),
        }
    }
}
