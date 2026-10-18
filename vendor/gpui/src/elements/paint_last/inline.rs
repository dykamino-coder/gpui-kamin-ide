//! Inline paint wrappers defer text until normal-flow block backgrounds finish.

use super::*;

/// KaminIDE patch: строчное содержимое блока (строки с текстом, строчными
/// коробками и атомами), которое рисуется ПОСЛЕ фонов и рамок всех блоков
/// потока своего контекста наложения.
///
/// CSS 2.1 прил. E: шаг 4 — фоны блочных потомков в порядке дерева, шаг 5 —
/// флоаты, шаг 7 — строчное содержимое (в порядке дерева, строка за
/// строкой), шаг 8 — позиционированные. Порядок краски в GPUI — порядок
/// детей, поэтому строки раннего блока ложились ПОД фон позднего блока,
/// наехавшего на них отрицательным полем (`inline-block-zorder-001`).
/// При открытом собирателе (`PaintCollect` корня, `Div` с прозрачностью)
/// обёртка уходит в него и рисуется в его конце раньше позиционированных;
/// без собирателя — на месте, как прежде. Раскладка не меняется.
pub struct PaintInline {
    child: Option<AnyElement>,
}

impl PaintInline {
    /// Обернуть строчное содержимое.
    pub fn new(child: AnyElement) -> Self {
        PaintInline { child: Some(child) }
    }
}

impl Element for PaintInline {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = self.child.as_mut().expect("PaintInline: ребёнок на месте");
        (child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = self.child.as_mut() {
            child.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if hoist_open() && FRAMES.with(|f| !f.borrow().is_empty()) {
            if let Some(el) = self.child.take() {
                let ctx = window.text_paint_ctx();
                FRAMES.with(|f| {
                    if let Some(frame) = f.borrow_mut().last_mut() {
                        frame.inline.push_back(HoistedInline { el, ctx });
                    }
                });
            }
            return;
        }
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for PaintInline {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
