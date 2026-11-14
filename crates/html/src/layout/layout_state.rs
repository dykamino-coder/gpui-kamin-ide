//! Per-document frame state of the layout phase: anchor registries, the prepaint transform
//! stack and remembered `position-try` choices (`AnchorState`), and the root's measured left
//! edge under `vertical-rl` (`root_left`). The document owns one shared instance and hands it
//! to its builds through `LayoutCx::layout`; an embedded document has its own. Elements that
//! read or write it at prepaint or paint retain this exact `Rc`, not the value context.

use crate::layout::positioned::anchor::AnchorState;
use std::cell::Cell;

#[derive(Default)]
pub(crate) struct LayoutState {
    /// Числовая геометрия контекстов с проверкой входов каждой ревизии.
    pub(crate) formatting: std::rc::Rc<crate::layout::formatting_cache::Store>,
    /// Anchor geometry and fallback selection belonging to this document.
    pub(crate) anchors: AnchorState,
    /// Факты неизменяемого дерева; новая computed-публикация их сбрасывает.
    pub(crate) preparation: crate::layout::cx::subtree::PreparationCache,
    /// Left edge of the root's content-sized box under `vertical-rl`
    /// (CSS 2.2 §14.2: the canvas background is positioned from the root
    /// box). The body writes it at prepaint, the canvas layer reads it at
    /// paint of the same frame; the previous frame's value is the fallback.
    pub(crate) root_left: Cell<Option<f32>>,
    /// Element styles converted in the current build of this document.
    pub(crate) conversions: crate::layout::cx::conversions::StyleConversions,
    /// Run fonts derived in the current build of this document.
    pub(crate) run_faces: crate::layout::cx::run_faces::RunFaces,
    /// Heap stacks of paragraph collections reused in the current build of this document.
    pub(crate) collect_stacks: crate::layout::cx::collect_stacks::CollectStacks,
}

impl LayoutState {
    /// Состояние ревизии, дерево которой — копия путей к правкам прошлого
    /// (`Document::adopted`): переходят подготовка дерева и проверяемые числовые
    /// результаты контекстов, рекордеры и геометрия кадра начинаются заново.
    pub(crate) fn continuing(previous: &Self) -> Self {
        Self {
            formatting: previous.formatting.clone(),
            preparation: crate::layout::cx::subtree::PreparationCache::continuing(
                &previous.preparation,
            ),
            ..Self::default()
        }
    }
}
