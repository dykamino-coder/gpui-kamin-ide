//! Обёртка `LayoutTap`, запоминающая узел раскладки ребёнка для ТОЧНОГО,
//! не округлённого к точке устройства, размера атома (`Window::layout_exact`).
//! Базовую линию атома ряд-обёртка берёт из его собственной раскладки
//! (`atoms::row_geometry`), отдельный щуп ей не нужен.

use crate::hooks::Wrap;
use gpui::{AnyElement, App, LayoutId, Window};

/// Обёртка, запоминающая узел раскладки своего ребёнка: по нему берётся
/// ТОЧНЫЙ размер атома (`Window::layout_exact`) — округлённый к точке
/// устройства прибавлял до 0.4px на атом, и ряд атомов ровно в ширину строки
/// в неё уже не влезал (`c542-letter-sp-001-ref`, `c5505-mrgn-000`).
pub(crate) struct LayoutTap {
    pub(crate) child: AnyElement,
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Wrap for LayoutTap {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> LayoutId {
        let id = self.child.request_layout_async(window, cx, stack).await;
        self.slot.set(Some(id));
        id
    }
}
